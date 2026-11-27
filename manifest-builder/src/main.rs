use std::collections::HashMap;
use std::io::Read;
use std::path::{Path as FsPath, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;

use axum::extract::{DefaultBodyLimit, Multipart, Path, Query, Request, State};
use axum::http::{header, StatusCode};
use axum::middleware::{self, Next};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use tokio::io::AsyncWriteExt;
use tokio::process::Command;

const ADDRESS: &str = "127.0.0.1:8080";
const URL: &str = "http://127.0.0.1:8080";
/// Кореневі папки в pack/. `saves/` — стартові світи: лаунчер докачує лише відсутні файли
/// й ніколи не перезаписує світ гравця.
const MANAGED_DIRS: [&str; 5] = ["mods", "config", "resourcepacks", "shaderpacks", "saves"];
/// Службові файли світу, які не потрапляють у manifest.json і на GitHub.
const WORLD_JUNK: [&str; 2] = ["session.lock", "level.dat_old"];
const CATEGORIES: [&str; 3] = ["mods", "shaderpacks", "resourcepacks"];
const CONFIG_EXTENSIONS: [&str; 7] = ["toml", "json", "cfg", "properties", "yml", "yaml", "txt"];
const GITHUB_FILE_LIMIT: u64 = 100 * 1024 * 1024;
const CONFIG_SIZE_LIMIT: u64 = 5 * 1024 * 1024;

#[derive(Clone)]
struct RepoInfo {
    owner: String,
    repo: String,
    branch: String,
}

struct AppState {
    /// Корінь git-репозиторію: тут виконуються git-команди й лежить .admin-token.
    root: PathBuf,
    /// Папка збірки з pack/ і manifest.json: instances/<name>/ або сам корінь.
    base: PathBuf,
    instance: Option<String>,
    remote: RepoInfo,
    client: reqwest::Client,
    lock: tokio::sync::Mutex<()>,
}

type Shared = Arc<AppState>;
type Failure = (StatusCode, Json<Value>);
type ApiResult = Result<Json<Value>, Failure>;

fn fail(message: impl ToString) -> Failure {
    (
        StatusCode::BAD_REQUEST,
        Json(json!({ "error": message.to_string() })),
    )
}

#[derive(Serialize, Deserialize, Clone)]
struct Meta {
    name: String,
    version: String,
    minecraft_version: String,
    loader_type: String,
    loader_version: String,
    java_major: Option<u32>,
    recommended_ram_mb: Option<u32>,
}

#[derive(Serialize)]
struct LoaderOut {
    #[serde(rename = "type")]
    kind: String,
    version: String,
}

#[derive(Serialize, Deserialize)]
struct FileOut {
    path: String,
    sha256: String,
    #[serde(default)]
    size: u64,
    url: String,
}

#[derive(Deserialize)]
struct PreviousManifest {
    #[serde(default)]
    files: Vec<FileOut>,
}

#[derive(Serialize)]
struct ManifestOut {
    name: String,
    version: String,
    minecraft_version: String,
    loader: LoaderOut,
    #[serde(skip_serializing_if = "Option::is_none")]
    java_major: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    recommended_ram_mb: Option<u32>,
    files: Vec<FileOut>,
}

#[derive(Deserialize)]
struct TokenBody {
    token: String,
}

#[derive(Deserialize)]
struct ConfigBody {
    path: String,
    content: String,
}

#[derive(Deserialize)]
struct ServerBody {
    name: String,
    ip: String,
}

enum Nbt {
    Byte(i8),
    Short(i16),
    Int(i32),
    Long(i64),
    Float(f32),
    Double(f64),
    ByteArray(Vec<i8>),
    String(String),
    List(u8, Vec<Nbt>),
    Compound(Vec<(String, Nbt)>),
    IntArray(Vec<i32>),
    LongArray(Vec<i64>),
}

struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn take(&mut self, count: usize) -> Result<&'a [u8], String> {
        let data: &'a [u8] = self.data;
        let end = self
            .pos
            .checked_add(count)
            .filter(|end| *end <= data.len())
            .ok_or_else(|| "несподіваний кінець NBT".to_string())?;
        let slice = &data[self.pos..end];
        self.pos = end;
        Ok(slice)
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], String> {
        let slice = self.take(N)?;
        let mut out = [0u8; N];
        out.copy_from_slice(slice);
        Ok(out)
    }

    fn length(&mut self) -> Result<usize, String> {
        let value = i32::from_be_bytes(self.array()?);
        Ok(if value < 0 { 0 } else { value as usize })
    }

    fn string(&mut self) -> Result<String, String> {
        let count = u16::from_be_bytes(self.array()?) as usize;
        Ok(decode_mutf8(self.take(count)?))
    }

    fn payload(&mut self, tag: u8) -> Result<Nbt, String> {
        Ok(match tag {
            1 => Nbt::Byte(self.take(1)?[0] as i8),
            2 => Nbt::Short(i16::from_be_bytes(self.array()?)),
            3 => Nbt::Int(i32::from_be_bytes(self.array()?)),
            4 => Nbt::Long(i64::from_be_bytes(self.array()?)),
            5 => Nbt::Float(f32::from_be_bytes(self.array()?)),
            6 => Nbt::Double(f64::from_be_bytes(self.array()?)),
            7 => {
                let count = self.length()?;
                let bytes = self.take(count)?;
                Nbt::ByteArray(bytes.iter().map(|b| *b as i8).collect())
            }
            8 => Nbt::String(self.string()?),
            9 => {
                let inner = self.take(1)?[0];
                let count = self.length()?;
                let mut items = Vec::new();
                for _ in 0..count {
                    items.push(self.payload(inner)?);
                }
                Nbt::List(inner, items)
            }
            10 => {
                let mut entries = Vec::new();
                loop {
                    let inner = self.take(1)?[0];
                    if inner == 0 {
                        break;
                    }
                    let name = self.string()?;
                    let value = self.payload(inner)?;
                    entries.push((name, value));
                }
                Nbt::Compound(entries)
            }
            11 => {
                let count = self.length()?;
                let mut items = Vec::new();
                for _ in 0..count {
                    items.push(i32::from_be_bytes(self.array()?));
                }
                Nbt::IntArray(items)
            }
            12 => {
                let count = self.length()?;
                let mut items = Vec::new();
                for _ in 0..count {
                    items.push(i64::from_be_bytes(self.array()?));
                }
                Nbt::LongArray(items)
            }
            other => return Err(format!("невідомий NBT-тег {other}")),
        })
    }
}

fn decode_mutf8(bytes: &[u8]) -> String {
    let mut units: Vec<u16> = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        if b < 0x80 {
            units.push(b as u16);
            i += 1;
        } else if b & 0xE0 == 0xC0 && i + 1 < bytes.len() {
            units.push((((b & 0x1F) as u16) << 6) | (bytes[i + 1] & 0x3F) as u16);
            i += 2;
        } else if b & 0xF0 == 0xE0 && i + 2 < bytes.len() {
            units.push(
                (((b & 0x0F) as u16) << 12)
                    | (((bytes[i + 1] & 0x3F) as u16) << 6)
                    | (bytes[i + 2] & 0x3F) as u16,
            );
            i += 3;
        } else {
            units.push(0xFFFD);
            i += 1;
        }
    }
    String::from_utf16_lossy(&units)
}

fn encode_mutf8(text: &str) -> Vec<u8> {
    let mut out = Vec::new();
    for unit in text.encode_utf16() {
        match unit {
            0x0001..=0x007F => out.push(unit as u8),
            0x0000 | 0x0080..=0x07FF => {
                out.push(0xC0 | (unit >> 6) as u8);
                out.push(0x80 | (unit & 0x3F) as u8);
            }
            _ => {
                out.push(0xE0 | (unit >> 12) as u8);
                out.push(0x80 | ((unit >> 6) & 0x3F) as u8);
                out.push(0x80 | (unit & 0x3F) as u8);
            }
        }
    }
    out
}

fn tag_id(value: &Nbt) -> u8 {
    match value {
        Nbt::Byte(_) => 1,
        Nbt::Short(_) => 2,
        Nbt::Int(_) => 3,
        Nbt::Long(_) => 4,
        Nbt::Float(_) => 5,
        Nbt::Double(_) => 6,
        Nbt::ByteArray(_) => 7,
        Nbt::String(_) => 8,
        Nbt::List(_, _) => 9,
        Nbt::Compound(_) => 10,
        Nbt::IntArray(_) => 11,
        Nbt::LongArray(_) => 12,
    }
}

fn write_string(out: &mut Vec<u8>, text: &str) {
    let bytes = encode_mutf8(text);
    let length = bytes.len().min(u16::MAX as usize);
    out.extend_from_slice(&(length as u16).to_be_bytes());
    out.extend_from_slice(&bytes[..length]);
}

fn write_payload(out: &mut Vec<u8>, value: &Nbt) {
    match value {
        Nbt::Byte(v) => out.push(*v as u8),
        Nbt::Short(v) => out.extend_from_slice(&v.to_be_bytes()),
        Nbt::Int(v) => out.extend_from_slice(&v.to_be_bytes()),
        Nbt::Long(v) => out.extend_from_slice(&v.to_be_bytes()),
        Nbt::Float(v) => out.extend_from_slice(&v.to_be_bytes()),
        Nbt::Double(v) => out.extend_from_slice(&v.to_be_bytes()),
        Nbt::ByteArray(items) => {
            out.extend_from_slice(&(items.len() as i32).to_be_bytes());
            out.extend(items.iter().map(|b| *b as u8));
        }
        Nbt::String(text) => write_string(out, text),
        Nbt::List(kind, items) => {
            out.push(*kind);
            out.extend_from_slice(&(items.len() as i32).to_be_bytes());
            for item in items {
                write_payload(out, item);
            }
        }
        Nbt::Compound(entries) => {
            for (name, item) in entries {
                out.push(tag_id(item));
                write_string(out, name);
                write_payload(out, item);
            }
            out.push(0);
        }
        Nbt::IntArray(items) => {
            out.extend_from_slice(&(items.len() as i32).to_be_bytes());
            for item in items {
                out.extend_from_slice(&item.to_be_bytes());
            }
        }
        Nbt::LongArray(items) => {
            out.extend_from_slice(&(items.len() as i32).to_be_bytes());
            for item in items {
                out.extend_from_slice(&item.to_be_bytes());
            }
        }
    }
}

fn encode_nbt(root_name: &str, root: &Nbt) -> Vec<u8> {
    let mut out = Vec::new();
    out.push(tag_id(root));
    write_string(&mut out, root_name);
    write_payload(&mut out, root);
    out
}

fn parse_nbt(bytes: &[u8]) -> Result<(String, Nbt), String> {
    let mut reader = Reader { data: bytes, pos: 0 };
    let tag = reader.take(1)?[0];
    if tag != 10 {
        return Err(
            "файл не схожий на servers.dat (очікується NBT Compound без стиснення)".to_string(),
        );
    }
    let name = reader.string()?;
    let root = reader.payload(10)?;
    Ok((name, root))
}

fn servers_mut(root: &mut Nbt) -> Result<&mut Vec<Nbt>, String> {
    let Nbt::Compound(entries) = root else {
        return Err("кореневий тег не є Compound".to_string());
    };
    if !entries.iter().any(|(key, _)| key == "servers") {
        entries.push(("servers".to_string(), Nbt::List(10, Vec::new())));
    }
    for (key, value) in entries.iter_mut() {
        if key == "servers" {
            if let Nbt::List(kind, items) = value {
                if items.is_empty() {
                    *kind = 10;
                }
                return Ok(items);
            }
            return Err("тег servers не є списком".to_string());
        }
    }
    Err("тег servers не знайдено".to_string())
}

fn server_field(item: &Nbt, key: &str) -> String {
    if let Nbt::Compound(entries) = item {
        for (name, value) in entries {
            if name == key {
                if let Nbt::String(text) = value {
                    return text.clone();
                }
            }
        }
    }
    String::new()
}

fn servers_path(base: &FsPath) -> PathBuf {
    base.join("pack").join("servers.dat")
}

async fn load_servers(path: &FsPath) -> Result<(String, Nbt), String> {
    match tokio::fs::read(path).await {
        Ok(bytes) => parse_nbt(&bytes),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            Ok((String::new(), Nbt::Compound(Vec::new())))
        }
        Err(error) => Err(error.to_string()),
    }
}

async fn save_servers(path: &FsPath, root_name: &str, root: &Nbt) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| e.to_string())?;
    }
    tokio::fs::write(path, encode_nbt(root_name, root))
        .await
        .map_err(|e| e.to_string())
}

fn hide(command: &mut Command) {
    #[cfg(windows)]
    {
        command.creation_flags(0x0800_0000);
    }
    #[cfg(not(windows))]
    {
        let _ = command;
    }
}

async fn git(root: &FsPath, args: &[&str]) -> Result<String, String> {
    let mut command = Command::new("git");
    command
        .args(args)
        .current_dir(root)
        .env("GIT_TERMINAL_PROMPT", "0")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    hide(&mut command);
    let output = command
        .output()
        .await
        .map_err(|e| format!("не вдалося запустити git: {e}"))?;
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let text = format!("{}\n{}", stdout.trim(), stderr.trim())
        .trim()
        .to_string();
    if output.status.success() {
        Ok(text)
    } else if text.is_empty() {
        Err("git завершився з помилкою".to_string())
    } else {
        Err(text)
    }
}

async fn read_token(root: &FsPath) -> Option<String> {
    let text = tokio::fs::read_to_string(root.join(".admin-token"))
        .await
        .ok()?;
    let token = text.trim().to_string();
    if token.is_empty() {
        None
    } else {
        Some(token)
    }
}

/// Лише stdout: попередження git у stderr не мають потрапити в URL чи шлях.
async fn git_stdout(dir: &FsPath, args: &[&str]) -> Result<String, String> {
    let mut command = Command::new("git");
    command.args(args).current_dir(dir).stdin(Stdio::null());
    hide(&mut command);
    let output = command
        .output()
        .await
        .map_err(|e| format!("не вдалося запустити git: {e}"))?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// `https://github.com/<owner>/<repo>[.git]` або `git@github.com:<owner>/<repo>[.git]`.
fn parse_github_url(url: &str) -> Option<(String, String)> {
    let url = url.trim();
    let path = match url.strip_prefix("git@github.com:") {
        Some(path) => path,
        None => {
            let (host, path) = url.strip_prefix("https://")?.split_once('/')?;
            // У URL може бути user:token@ перед хостом.
            if host.rsplit('@').next() != Some("github.com") {
                return None;
            }
            path
        }
    };
    let path = path.trim_end_matches('/');
    let path = path.strip_suffix(".git").unwrap_or(path);
    let (owner, repo) = path.split_once('/')?;
    let valid = |part: &str| {
        !part.is_empty()
            && part
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
    };
    (valid(owner) && valid(repo)).then(|| (owner.to_string(), repo.to_string()))
}

async fn detect_remote(root: &FsPath) -> Result<RepoInfo, String> {
    let url = git_stdout(root, &["remote", "get-url", "origin"])
        .await
        .map_err(|e| format!("не вдалося отримати URL origin: {e}"))?;
    // Сам URL не друкуємо: у ньому може бути токен.
    let (owner, repo) = parse_github_url(&url).ok_or_else(|| {
        "origin не схожий на репозиторій GitHub (https://github.com/<owner>/<repo> або git@github.com:<owner>/<repo>)"
            .to_string()
    })?;
    let branch = git_stdout(root, &["branch", "--show-current"])
        .await
        .ok()
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| "main".to_string());
    Ok(RepoInfo {
        owner,
        repo,
        branch,
    })
}

async fn remote_op(root: &FsPath, remote: &RepoInfo, op: &str) -> Result<String, String> {
    let mut args: Vec<&str> = vec![op];
    if op == "pull" {
        args.push("--ff-only");
    }
    args.push("origin");
    args.push(&remote.branch);
    let first = match git(root, &args).await {
        Ok(text) => return Ok(text),
        Err(error) => error,
    };
    let Some(token) = read_token(root).await else {
        return Err(first);
    };
    let url = format!(
        "https://x-access-token:{token}@github.com/{}/{}.git",
        remote.owner, remote.repo
    );
    let mut args: Vec<&str> = vec![op];
    if op == "pull" {
        args.push("--ff-only");
    }
    args.push(&url);
    args.push(&remote.branch);
    git(root, &args)
        .await
        .map_err(|error| error.replace(&token, "***"))
}

async fn github_login(client: &reqwest::Client, token: &str) -> Result<String, String> {
    let response = client
        .get("https://api.github.com/user")
        .header("Authorization", format!("Bearer {token}"))
        .header("User-Agent", "manifest-builder")
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .map_err(|e| format!("немає зв'язку з GitHub: {e}"))?;
    if !response.status().is_success() {
        return Err(format!("GitHub відхилив токен ({})", response.status()));
    }
    let value: Value = response.json().await.map_err(|e| e.to_string())?;
    value
        .get("login")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| "у відповіді GitHub немає login".to_string())
}

fn is_ignored(name: &str) -> bool {
    let lower = name.to_lowercase();
    name.starts_with('.')
        || lower.ends_with(".part")
        || lower == "thumbs.db"
        || lower == "desktop.ini"
}

/// `saves/<світ>/.../session.lock` чи `level.dat_old` — тимчасові файли гри, їх не публікуємо.
fn is_world_junk(relative: &str) -> bool {
    relative.starts_with("saves/")
        && relative
            .rsplit('/')
            .next()
            .map(|name| WORLD_JUNK.iter().any(|junk| junk.eq_ignore_ascii_case(name)))
            .unwrap_or(false)
}

/// Те саме правило, що й у лаунчері (manifest::safe_relative): лише Normal-компоненти
/// (жодних `..`, `.`, абсолютних шляхів чи дисків) і лише дозволені кореневі папки.
fn is_allowed_path(relative: &str) -> bool {
    if relative == "servers.dat" {
        return true;
    }
    let parts: Vec<&str> = relative.split('/').collect();
    let clean = parts.iter().all(|part| {
        !part.is_empty() && *part != "." && *part != ".." && !part.contains('\\') && !part.contains(':')
    });
    clean && parts.len() >= 2 && MANAGED_DIRS.contains(&parts[0])
}

fn walk(base: &FsPath) -> Result<Vec<(String, u64)>, String> {
    let mut out = Vec::new();
    if !base.is_dir() {
        return Ok(out);
    }
    let mut stack = vec![(base.to_path_buf(), String::new())];
    while let Some((dir, prefix)) = stack.pop() {
        let entries = std::fs::read_dir(&dir).map_err(|e| e.to_string())?;
        for entry in entries {
            let entry = entry.map_err(|e| e.to_string())?;
            let name = entry.file_name().to_string_lossy().to_string();
            if is_ignored(&name) {
                continue;
            }
            let relative = if prefix.is_empty() {
                name.clone()
            } else {
                format!("{prefix}/{name}")
            };
            let file_type = entry.file_type().map_err(|e| e.to_string())?;
            if file_type.is_dir() {
                stack.push((entry.path(), relative));
            } else if file_type.is_file() {
                let size = entry.metadata().map_err(|e| e.to_string())?.len();
                out.push((relative, size));
            }
        }
    }
    out.sort_by(|a, b| a.0.to_lowercase().cmp(&b.0.to_lowercase()));
    Ok(out)
}

fn safe_join(base: &FsPath, relative: &str) -> Result<PathBuf, Failure> {
    let mut out = base.to_path_buf();
    for part in relative.split('/') {
        if part.is_empty()
            || part == "."
            || part == ".."
            || part.contains('\\')
            || part.contains(':')
        {
            return Err(fail("небезпечний шлях"));
        }
        out.push(part);
    }
    Ok(out)
}

fn category_dir(base: &FsPath, category: &str) -> Result<PathBuf, Failure> {
    if CATEGORIES.contains(&category) {
        Ok(base.join("pack").join(category))
    } else {
        Err(fail("невідома категорія"))
    }
}

fn is_config_file(name: &str) -> bool {
    let lower = name.to_lowercase();
    CONFIG_EXTENSIONS
        .iter()
        .any(|ext| lower.ends_with(&format!(".{ext}")))
}

fn config_path(base: &FsPath, relative: &str) -> Result<PathBuf, Failure> {
    if !is_config_file(relative) {
        return Err(fail("непідтримуваний тип файлу"));
    }
    safe_join(&base.join("pack").join("config"), relative)
}

fn encode_segment(segment: &str) -> String {
    let mut out = String::with_capacity(segment.len());
    for byte in segment.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                out.push(byte as char)
            }
            other => out.push_str(&format!("%{other:02X}")),
        }
    }
    out
}

fn encode_path(path: &str) -> String {
    path.split('/')
        .map(encode_segment)
        .collect::<Vec<_>>()
        .join("/")
}

fn hash_file(path: &FsPath) -> Result<String, String> {
    let mut file =
        std::fs::File::open(path).map_err(|e| format!("не вдалося відкрити {}: {e}", path.display()))?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; 65536];
    loop {
        let read = file.read(&mut buffer).map_err(|e| e.to_string())?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hex::encode(hasher.finalize()))
}

fn raw_base(remote: &RepoInfo, instance: Option<&str>) -> String {
    let prefix = match instance {
        Some(name) => format!("instances/{}/pack", encode_segment(name)),
        None => "pack".to_string(),
    };
    format!(
        "https://raw.githubusercontent.com/{}/{}/{}/{prefix}",
        remote.owner,
        remote.repo,
        encode_path(&remote.branch)
    )
}

/// Записи поточного manifest.json на файли з GitHub Releases, яких немає в pack/.
fn release_entries(base: &FsPath) -> Result<Vec<FileOut>, String> {
    let text = match std::fs::read_to_string(base.join("manifest.json")) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(format!("не вдалося прочитати manifest.json: {error}")),
    };
    // Пошкоджений маніфест не перезаписуємо — інакше релізні записи зникнуть.
    let previous: PreviousManifest = serde_json::from_str(&text)
        .map_err(|e| format!("поточний manifest.json пошкоджений, виправ його: {e}"))?;
    let pack = base.join("pack");
    Ok(previous
        .files
        .into_iter()
        .filter(|file| file.url.contains("/releases/download/") && !pack.join(&file.path).is_file())
        .collect())
}

fn scan_pack(base: &FsPath, raw_base: &str) -> Result<Vec<FileOut>, String> {
    let pack = base.join("pack");
    let found = walk(&pack)?;
    let released = release_entries(base)?;
    if found.is_empty() && released.is_empty() {
        return Err("папка pack порожня".to_string());
    }
    let mut files = Vec::with_capacity(found.len());
    for (relative, size) in found {
        if is_world_junk(&relative) {
            continue;
        }
        if !is_allowed_path(&relative) {
            return Err(format!("файл поза дозволеними місцями: pack/{relative}"));
        }
        if size >= GITHUB_FILE_LIMIT {
            return Err(format!(
                "pack/{relative} ({} МБ) більший за 100 МБ, GitHub його не прийме. Завантаж файл у GitHub Releases, додай запис з цим посиланням у manifest.json і прибери файл з pack/",
                size / (1024 * 1024)
            ));
        }
        let sha256 = hash_file(&pack.join(&relative))?;
        files.push(FileOut {
            url: format!("{raw_base}/{}", encode_path(&relative)),
            path: relative,
            sha256,
            size,
        });
    }
    files.extend(released);
    files.sort_by_key(|file| file.path.to_lowercase());
    Ok(files)
}

fn safe_token(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_' | '+'))
}

fn validate_meta(meta: &Meta) -> Result<(), String> {
    if meta.name.trim().is_empty() || meta.name.chars().count() > 64 {
        return Err("назва збірки: від 1 до 64 символів".to_string());
    }
    if !safe_token(meta.minecraft_version.trim()) {
        return Err("некоректна версія Minecraft".to_string());
    }
    if !["vanilla", "fabric", "forge", "neoforge"].contains(&meta.loader_type.as_str()) {
        return Err("невідомий завантажувач".to_string());
    }
    let loader_version = meta.loader_version.trim();
    if meta.loader_type != "vanilla" && loader_version.is_empty() {
        return Err("вкажи версію завантажувача".to_string());
    }
    if !loader_version.is_empty() && !safe_token(loader_version) {
        return Err("некоректна версія завантажувача".to_string());
    }
    if let Some(java) = meta.java_major {
        if !(8..=40).contains(&java) {
            return Err("версія Java має бути від 8 до 40".to_string());
        }
    }
    if let Some(ram) = meta.recommended_ram_mb {
        if !(1024..=65536).contains(&ram) {
            return Err("RAM має бути від 1024 до 65536 МБ".to_string());
        }
    }
    Ok(())
}

fn next_version(current: &str) -> String {
    let parts: Vec<&str> = current.trim().split('.').collect();
    if parts.len() == 3 {
        if let (Ok(major), Ok(minor), Ok(patch)) = (
            parts[0].parse::<u64>(),
            parts[1].parse::<u64>(),
            parts[2].parse::<u64>(),
        ) {
            return format!("{major}.{minor}.{}", patch + 1);
        }
    }
    "1.0.0".to_string()
}

async fn read_meta(base: &FsPath) -> Meta {
    let value = match tokio::fs::read_to_string(base.join("manifest.json")).await {
        Ok(text) => serde_json::from_str::<Value>(&text).unwrap_or(Value::Null),
        Err(_) => Value::Null,
    };
    let text = |pointer: &str, fallback: &str| -> String {
        value
            .pointer(pointer)
            .and_then(|v| v.as_str())
            .unwrap_or(fallback)
            .to_string()
    };
    let number = |pointer: &str| -> Option<u32> {
        value
            .pointer(pointer)
            .and_then(|v| v.as_u64())
            .map(|n| n as u32)
    };
    Meta {
        name: text("/name", "FCL"),
        version: text("/version", ""),
        minecraft_version: text("/minecraft_version", "1.20.1"),
        loader_type: text("/loader/type", "fabric"),
        loader_version: text("/loader/version", ""),
        java_major: number("/java_major"),
        recommended_ram_mb: number("/recommended_ram_mb"),
    }
}

async fn guard(request: Request, next: Next) -> Response {
    let host_ok = request
        .headers()
        .get(header::HOST)
        .and_then(|v| v.to_str().ok())
        .map(|h| h == "127.0.0.1:8080" || h == "localhost:8080")
        .unwrap_or(false);
    let origin_ok = match request.headers().get(header::ORIGIN) {
        None => true,
        Some(value) => value
            .to_str()
            .map(|o| o == "http://127.0.0.1:8080" || o == "http://localhost:8080")
            .unwrap_or(false),
    };
    if host_ok && origin_ok {
        next.run(request).await
    } else {
        StatusCode::FORBIDDEN.into_response()
    }
}

async fn index() -> Html<&'static str> {
    Html(include_str!("index.html"))
}

async fn info(State(state): State<Shared>) -> ApiResult {
    Ok(Json(json!({
        "owner": state.remote.owner,
        "repo": state.remote.repo,
        "branch": state.remote.branch,
        "instance": state.instance,
    })))
}

async fn auth_status(State(state): State<Shared>) -> ApiResult {
    let Some(token) = read_token(&state.root).await else {
        return Ok(Json(json!({ "logged_in": false })));
    };
    match github_login(&state.client, &token).await {
        Ok(login) => Ok(Json(json!({ "logged_in": true, "login": login }))),
        Err(error) => Ok(Json(json!({ "logged_in": false, "error": error }))),
    }
}

async fn auth_login(State(state): State<Shared>, Json(body): Json<TokenBody>) -> ApiResult {
    let token = body.token.trim().to_string();
    if token.is_empty() || token.chars().any(|c| c.is_whitespace()) {
        return Err(fail("токен порожній або містить пробіли"));
    }
    let login = github_login(&state.client, &token)
        .await
        .map_err(|e| fail(e))?;
    tokio::fs::write(state.root.join(".admin-token"), &token)
        .await
        .map_err(|e| fail(e))?;
    Ok(Json(json!({ "login": login })))
}

async fn auth_logout(State(state): State<Shared>) -> ApiResult {
    let _ = tokio::fs::remove_file(state.root.join(".admin-token")).await;
    Ok(Json(json!({ "ok": true })))
}

async fn sync(State(state): State<Shared>) -> ApiResult {
    let _guard = state.lock.lock().await;
    let output = remote_op(&state.root, &state.remote, "pull")
        .await
        .map_err(|e| fail(format!("git pull: {e}")))?;
    let message = if output.is_empty() {
        "Готово".to_string()
    } else {
        output
    };
    Ok(Json(json!({ "message": message })))
}

async fn get_meta(State(state): State<Shared>) -> ApiResult {
    let meta = read_meta(&state.base).await;
    Ok(Json(json!(meta)))
}

async fn list_files(State(state): State<Shared>, Path(category): Path<String>) -> ApiResult {
    let dir = category_dir(&state.base, &category)?;
    let items = tokio::task::spawn_blocking(move || walk(&dir))
        .await
        .map_err(|e| fail(e))?
        .map_err(|e| fail(e))?;
    let list: Vec<Value> = items
        .into_iter()
        .map(|(name, size)| json!({ "name": name, "size": size }))
        .collect();
    Ok(Json(json!(list)))
}

async fn upload_files(
    State(state): State<Shared>,
    Path(category): Path<String>,
    mut multipart: Multipart,
) -> ApiResult {
    let dir = category_dir(&state.base, &category)?;
    tokio::fs::create_dir_all(&dir)
        .await
        .map_err(|e| fail(e))?;
    let mut saved: Vec<String> = Vec::new();
    while let Some(mut field) = multipart.next_field().await.map_err(|e| fail(e))? {
        let Some(file_name) = field.file_name().map(|s| s.to_string()) else {
            continue;
        };
        let name = file_name
            .rsplit(|c: char| c == '/' || c == '\\')
            .next()
            .unwrap_or("")
            .trim()
            .to_string();
        if name.is_empty() || is_ignored(&name) || name.contains(':') {
            return Err(fail(format!("недопустиме ім'я файлу: {file_name}")));
        }
        let target = dir.join(&name);
        let temp = dir.join(format!("{name}.part"));
        let mut file = tokio::fs::File::create(&temp)
            .await
            .map_err(|e| fail(e))?;
        while let Some(chunk) = field.chunk().await.map_err(|e| fail(e))? {
            file.write_all(&chunk).await.map_err(|e| fail(e))?;
        }
        file.flush().await.map_err(|e| fail(e))?;
        drop(file);
        tokio::fs::rename(&temp, &target)
            .await
            .map_err(|e| fail(e))?;
        saved.push(name);
    }
    Ok(Json(json!({ "saved": saved })))
}

async fn delete_file(
    State(state): State<Shared>,
    Path(category): Path<String>,
    Query(query): Query<HashMap<String, String>>,
) -> ApiResult {
    let dir = category_dir(&state.base, &category)?;
    let name = query
        .get("name")
        .ok_or_else(|| fail("не вказано name"))?;
    let path = safe_join(&dir, name)?;
    if !path.is_file() {
        return Err(fail("файл не знайдено"));
    }
    tokio::fs::remove_file(&path).await.map_err(|e| fail(e))?;
    Ok(Json(json!({ "deleted": name })))
}

async fn list_configs(State(state): State<Shared>) -> ApiResult {
    let dir = state.base.join("pack").join("config");
    let items = tokio::task::spawn_blocking(move || walk(&dir))
        .await
        .map_err(|e| fail(e))?
        .map_err(|e| fail(e))?;
    let list: Vec<Value> = items
        .into_iter()
        .filter(|(name, _)| is_config_file(name))
        .map(|(name, size)| json!({ "name": name, "size": size }))
        .collect();
    Ok(Json(json!(list)))
}

async fn read_config(
    State(state): State<Shared>,
    Query(query): Query<HashMap<String, String>>,
) -> ApiResult {
    let relative = query
        .get("path")
        .ok_or_else(|| fail("не вказано path"))?;
    let path = config_path(&state.base, relative)?;
    let metadata = tokio::fs::metadata(&path)
        .await
        .map_err(|e| fail(format!("не вдалося відкрити файл: {e}")))?;
    if metadata.len() > CONFIG_SIZE_LIMIT {
        return Err(fail("файл завеликий для редактора (понад 5 МБ)"));
    }
    let content = tokio::fs::read_to_string(&path)
        .await
        .map_err(|e| fail(format!("не вдалося прочитати файл як UTF-8: {e}")))?;
    Ok(Json(json!({ "content": content })))
}

async fn write_config(State(state): State<Shared>, Json(body): Json<ConfigBody>) -> ApiResult {
    let path = config_path(&state.base, &body.path)?;
    if !path.is_file() {
        return Err(fail("файл не існує"));
    }
    tokio::fs::write(&path, body.content)
        .await
        .map_err(|e| fail(e))?;
    Ok(Json(json!({ "saved": body.path })))
}

async fn list_servers(State(state): State<Shared>) -> ApiResult {
    let path = servers_path(&state.base);
    let (_, mut root) = load_servers(&path).await.map_err(|e| fail(e))?;
    let items = servers_mut(&mut root).map_err(|e| fail(e))?;
    let list: Vec<Value> = items
        .iter()
        .map(|item| json!({ "name": server_field(item, "name"), "ip": server_field(item, "ip") }))
        .collect();
    Ok(Json(json!(list)))
}

async fn add_server(State(state): State<Shared>, Json(body): Json<ServerBody>) -> ApiResult {
    let name = body.name.trim().to_string();
    let ip = body.ip.trim().to_string();
    if name.is_empty() || ip.is_empty() {
        return Err(fail("потрібні назва та IP"));
    }
    if name.chars().count() > 64 || ip.chars().count() > 128 {
        return Err(fail("задовга назва або адреса"));
    }
    let _guard = state.lock.lock().await;
    let path = servers_path(&state.base);
    let (root_name, mut root) = load_servers(&path).await.map_err(|e| fail(e))?;
    servers_mut(&mut root)
        .map_err(|e| fail(e))?
        .push(Nbt::Compound(vec![
            ("ip".to_string(), Nbt::String(ip)),
            ("name".to_string(), Nbt::String(name)),
        ]));
    save_servers(&path, &root_name, &root)
        .await
        .map_err(|e| fail(e))?;
    Ok(Json(json!({ "ok": true })))
}

async fn delete_server(
    State(state): State<Shared>,
    Query(query): Query<HashMap<String, String>>,
) -> ApiResult {
    let index: usize = query
        .get("index")
        .and_then(|v| v.parse().ok())
        .ok_or_else(|| fail("некоректний index"))?;
    let _guard = state.lock.lock().await;
    let path = servers_path(&state.base);
    let (root_name, mut root) = load_servers(&path).await.map_err(|e| fail(e))?;
    let items = servers_mut(&mut root).map_err(|e| fail(e))?;
    if index >= items.len() {
        return Err(fail("сервера з таким номером немає"));
    }
    items.remove(index);
    save_servers(&path, &root_name, &root)
        .await
        .map_err(|e| fail(e))?;
    Ok(Json(json!({ "ok": true })))
}

async fn publish(State(state): State<Shared>, Json(meta): Json<Meta>) -> ApiResult {
    let _guard = state.lock.lock().await;
    validate_meta(&meta).map_err(|e| fail(e))?;
    ensure_gitattributes(&state.root)
        .await
        .map_err(|e| fail(format!(".gitattributes: {e}")))?;

    let previous = read_meta(&state.base).await;
    let version = next_version(&previous.version);

    let scan_base = state.base.clone();
    let url_base = raw_base(&state.remote, state.instance.as_deref());
    let files = tokio::task::spawn_blocking(move || scan_pack(&scan_base, &url_base))
        .await
        .map_err(|e| fail(e))?
        .map_err(|e| fail(e))?;
    let count = files.len();

    let manifest = ManifestOut {
        name: meta.name.trim().to_string(),
        version: version.clone(),
        minecraft_version: meta.minecraft_version.trim().to_string(),
        loader: LoaderOut {
            kind: meta.loader_type.clone(),
            version: meta.loader_version.trim().to_string(),
        },
        java_major: meta.java_major,
        recommended_ram_mb: meta.recommended_ram_mb,
        files,
    };
    let text = serde_json::to_string_pretty(&manifest).map_err(|e| fail(e))?;
    tokio::fs::write(state.base.join("manifest.json"), text)
        .await
        .map_err(|e| fail(e))?;

    let mut log = vec![format!(
        "manifest.json оновлено: версія {version}, файлів {count}"
    )];

    // Лише ця збірка: незавершені зміни інших збірок не потрапляють у коміт.
    let mut paths: Vec<String> = vec![".gitattributes".to_string()];
    match &state.instance {
        Some(name) => paths.push(format!("instances/{name}")),
        None => {
            paths.push("pack".to_string());
            paths.push("manifest.json".to_string());
        }
    }
    if state.root.join("projects.json").is_file() {
        paths.push("projects.json".to_string());
    }
    let pathspec: Vec<&str> = paths.iter().map(String::as_str).collect();

    // --renormalize перезаписує в індексі вже відстежені файли за правилом -text,
    // тож на GitHub потрапляють ті самі байти, що й захешовані вище.
    let mut add_args = vec!["add", "--renormalize", "--"];
    add_args.extend(&pathspec);
    git(&state.root, &add_args)
        .await
        .map_err(|e| fail(format!("git add --renormalize: {e}")))?;
    let mut add_args = vec!["add", "-A", "--"];
    add_args.extend(&pathspec);
    git(&state.root, &add_args)
        .await
        .map_err(|e| fail(format!("git add: {e}")))?;
    log.push(format!("git add {}: ok", paths.join(" ")));

    let mut commit_args = vec!["commit", "-m", "Update pack", "--"];
    commit_args.extend(&pathspec);
    match git(&state.root, &commit_args).await {
        Ok(_) => log.push("git commit -m \"Update pack\": ok".to_string()),
        Err(error)
            if error.contains("nothing to commit") || error.contains("no changes added") =>
        {
            log.push("git commit: немає змін для коміту".to_string())
        }
        Err(error) => return Err(fail(format!("git commit: {error}"))),
    }

    remote_op(&state.root, &state.remote, "push")
        .await
        .map_err(|e| fail(format!("git push: {e}")))?;
    log.push(format!("git push origin {}: ok", state.remote.branch));

    Ok(Json(json!({ "version": version, "files": count, "log": log })))
}

struct Args {
    dir: PathBuf,
    instance: Option<String>,
}

fn parse_args(mut args: impl Iterator<Item = String>) -> Result<Args, String> {
    let mut dir = None;
    let mut instance = None;
    while let Some(arg) = args.next() {
        let value = if arg == "--instance" {
            Some(args.next().unwrap_or_default())
        } else {
            arg.strip_prefix("--instance=").map(str::to_string)
        };
        if let Some(value) = value {
            if value.is_empty() || value.starts_with('-') {
                return Err("після --instance вкажи назву збірки".to_string());
            }
            if instance.replace(value).is_some() {
                return Err("--instance вказано двічі".to_string());
            }
        } else if arg.starts_with('-') {
            return Err(format!("невідомий прапорець {arg}"));
        } else if dir.replace(PathBuf::from(&arg)).is_some() {
            return Err(format!("зайвий аргумент {arg}"));
        }
    }
    Ok(Args {
        dir: dir.unwrap_or_else(|| PathBuf::from(".")),
        instance,
    })
}

async fn find_root(dir: &FsPath) -> Result<PathBuf, String> {
    git_stdout(dir, &["rev-parse", "--show-toplevel"])
        .await
        .map(PathBuf::from)
        .map_err(|error| format!("{} не є git-репозиторієм: {error}", dir.display()))
}

fn list_instances(root: &FsPath) -> Result<Vec<String>, String> {
    let entries = std::fs::read_dir(root.join("instances")).map_err(|e| e.to_string())?;
    let mut names = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|e| e.to_string())?;
        let name = entry.file_name().to_string_lossy().to_string();
        if entry.path().is_dir() && !name.starts_with('.') {
            names.push(name);
        }
    }
    names.sort();
    Ok(names)
}

/// Повертає папку збірки. Нічого не створює і не змінює.
fn resolve_base(root: &FsPath, instance: Option<&str>) -> Result<PathBuf, String> {
    let has_instances = root.join("instances").is_dir();
    let available = if has_instances {
        list_instances(root)?
    } else {
        Vec::new()
    };
    let listing = if available.is_empty() {
        "  (немає жодної)".to_string()
    } else {
        available
            .iter()
            .map(|name| format!("  {name}"))
            .collect::<Vec<_>>()
            .join("\n")
    };
    match instance {
        None if has_instances => Err(format!(
            "вкажи збірку: manifest-builder --instance <name>\nЗбірки в instances/:\n{listing}"
        )),
        None => Ok(root.to_path_buf()),
        Some(name) if available.iter().any(|item| item == name) => {
            Ok(root.join("instances").join(name))
        }
        Some(name) => Err(format!(
            "збірки \"{name}\" немає в instances/. Наявні збірки:\n{listing}"
        )),
    }
}

async fn ensure_ignored(root: &FsPath) {
    let path = root.join(".gitignore");
    let current = tokio::fs::read_to_string(&path).await.unwrap_or_default();
    if current.lines().any(|line| line.trim() == ".admin-token") {
        return;
    }
    let mut next = current;
    if !next.is_empty() && !next.ends_with('\n') {
        next.push('\n');
    }
    next.push_str(".admin-token\n");
    let _ = tokio::fs::write(&path, next).await;
}

/// Вимикає конвертацію CRLF→LF для файлів збірки: інакше git (core.autocrlf)
/// змінює байти й SHA-256 у manifest.json перестає збігатися з файлом на GitHub.
async fn ensure_gitattributes(root: &FsPath) -> Result<(), String> {
    let rule = if root.join("instances").is_dir() {
        "instances/*/pack/** -text"
    } else {
        "pack/** -text"
    };
    let path = root.join(".gitattributes");
    let current = match tokio::fs::read_to_string(&path).await {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(error) => return Err(error.to_string()),
    };
    let normalized = |line: &str| line.split_whitespace().collect::<Vec<_>>().join(" ");
    if current.lines().any(|line| normalized(line) == rule) {
        return Ok(());
    }
    let mut next = current;
    if !next.is_empty() && !next.ends_with('\n') {
        next.push('\n');
    }
    next.push_str(rule);
    next.push('\n');
    tokio::fs::write(&path, next)
        .await
        .map_err(|e| e.to_string())
}

fn open_browser(url: &str) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let _ = std::process::Command::new("cmd")
            .args(["/C", "start", "", url])
            .creation_flags(0x0800_0000)
            .spawn();
    }
    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("open").arg(url).spawn();
    }
    #[cfg(all(not(windows), not(target_os = "macos")))]
    {
        let _ = std::process::Command::new("xdg-open").arg(url).spawn();
    }
}

async fn setup() -> Result<(PathBuf, PathBuf, Option<String>, RepoInfo), String> {
    let args = parse_args(std::env::args().skip(1))?;
    let root = find_root(&args.dir).await?;
    let base = resolve_base(&root, args.instance.as_deref())?;
    let remote = detect_remote(&root).await?;
    Ok((root, base, args.instance, remote))
}

#[tokio::main]
async fn main() {
    let (root, base, instance, remote) = match setup().await {
        Ok(found) => found,
        Err(message) => {
            eprintln!("помилка: {message}");
            std::process::exit(1);
        }
    };

    for dir in MANAGED_DIRS {
        let _ = tokio::fs::create_dir_all(base.join("pack").join(dir)).await;
    }
    ensure_ignored(&root).await;
    if let Err(error) = ensure_gitattributes(&root).await {
        eprintln!("попередження: не вдалося оновити .gitattributes: {error}");
    }

    let client = match reqwest::Client::builder()
        .timeout(Duration::from_secs(20))
        .build()
    {
        Ok(client) => client,
        Err(error) => {
            eprintln!("помилка HTTP-клієнта: {error}");
            std::process::exit(1);
        }
    };

    let shown = root.display().to_string();
    let target = format!(
        "{}/{} · {} · {}",
        remote.owner,
        remote.repo,
        remote.branch,
        instance.as_deref().unwrap_or("корінь репозиторію")
    );
    let state: Shared = Arc::new(AppState {
        root,
        base,
        instance,
        remote,
        client,
        lock: tokio::sync::Mutex::new(()),
    });

    let app = Router::new()
        .route("/", get(index))
        .route("/api/info", get(info))
        .route(
            "/api/auth",
            get(auth_status).post(auth_login).delete(auth_logout),
        )
        .route("/api/sync", post(sync))
        .route("/api/meta", get(get_meta))
        .route(
            "/api/files/:category",
            get(list_files).post(upload_files).delete(delete_file),
        )
        .route("/api/config", get(list_configs))
        .route("/api/config/file", get(read_config).put(write_config))
        .route(
            "/api/servers",
            get(list_servers).post(add_server).delete(delete_server),
        )
        .route("/api/publish", post(publish))
        .layer(DefaultBodyLimit::disable())
        .layer(middleware::from_fn(guard))
        .with_state(state);

    let listener = match tokio::net::TcpListener::bind(ADDRESS).await {
        Ok(listener) => listener,
        Err(error) => {
            eprintln!("не вдалося зайняти {ADDRESS}: {error}");
            std::process::exit(1);
        }
    };

    println!("Адмін-панель: {URL}");
    println!("Репозиторій: {shown}");
    println!("Публікація: {target}");
    open_browser(URL);

    if let Err(error) = axum::serve(listener, app).await {
        eprintln!("помилка сервера: {error}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saves_are_allowed_without_traversal() {
        for path in [
            "saves/HW25/level.dat",
            "saves/WINTER DebtHunt v0.5/region/r.0.0.mca",
            "mods/a.jar",
            "servers.dat",
        ] {
            assert!(is_allowed_path(path), "{path}");
        }
        for path in [
            "saves",
            "saves/../mods/a.jar",
            "saves/./w/level.dat",
            "saves//level.dat",
            "C:/saves/w/level.dat",
            "options.txt",
            "logs/latest.log",
        ] {
            assert!(!is_allowed_path(path), "{path}");
        }
    }

    #[test]
    fn world_junk_is_skipped() {
        assert!(is_world_junk("saves/HW25/session.lock"));
        assert!(is_world_junk("saves/w/level.dat_old"));
        assert!(!is_world_junk("saves/w/level.dat"));
        assert!(!is_world_junk("config/session.lock"));
    }

    #[test]
    fn scan_pack_skips_world_junk() {
        let base = std::env::temp_dir().join(format!("mb-scan-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let world = base.join("pack").join("saves").join("W");
        std::fs::create_dir_all(&world).unwrap();
        std::fs::write(world.join("level.dat"), b"level").unwrap();
        std::fs::write(world.join("level.dat_old"), b"old").unwrap();
        std::fs::write(world.join("session.lock"), b"lock").unwrap();
        let files = scan_pack(&base, "https://raw.githubusercontent.com/o/r/main/pack").unwrap();
        let paths: Vec<&str> = files.iter().map(|f| f.path.as_str()).collect();
        assert_eq!(paths, vec!["saves/W/level.dat"]);
        let _ = std::fs::remove_dir_all(&base);
    }
}
