//! Оновлення лаунчера: інформація про реліз (версія, дата, повний опис змін) з
//! [`UPDATE_MIRRORS`](crate::constants::UPDATE_MIRRORS) і окреме встановлення .exe.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use reqwest::Client;
use semver::Version;
use serde::Serialize;
use serde_json::Value;
use tauri::AppHandle;

use crate::error::{fail, Context, Result};
use crate::events::{status, Progress};
use crate::i18n::tr;
use crate::mirrors::{self, Request};
use crate::t;
use crate::util::{download_file, Checksum};

const MAX_NOTES: usize = 40_000;

/// Те, що бачить гравець у сповіщенні та вікні «Що нового».
#[derive(Serialize, Clone, Debug, Default)]
pub struct ReleaseInfo {
    /// Версія без префікса `v`: "0.3.0".
    pub version: String,
    pub tag: String,
    pub name: String,
    /// Дата релізу (ISO 8601), може бути порожньою.
    pub date: String,
    /// Повний опис змін (Markdown з GitHub Releases або `notes` з update.json).
    pub notes: String,
    /// Посилання на .exe (оригінальне; дзеркала підставляються під час завантаження).
    pub url: String,
    pub size: u64,
    pub sha256: String,
    /// Звідки отримано інформацію (для діагностики).
    pub source: String,
}

pub enum UpdateOutcome {
    UpToDate,
    Restarting,
}

pub fn current_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

fn exe_sibling(exe: &Path, suffix: &str) -> PathBuf {
    let stem = exe
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("launcher");
    exe.with_file_name(format!("{stem}.{suffix}.exe"))
}

pub fn cleanup_old_executable() {
    std::thread::spawn(|| {
        let Ok(exe) = std::env::current_exe() else {
            return;
        };
        let old = exe_sibling(&exe, "old");
        for _ in 0..20 {
            if !old.exists() || std::fs::remove_file(&old).is_ok() {
                break;
            }
            std::thread::sleep(Duration::from_millis(500));
        }
    });
}

pub fn parse_version(raw: &str) -> Option<Version> {
    let cleaned = raw
        .trim()
        .trim_start_matches(|c: char| c == 'v' || c == 'V');
    if let Ok(version) = Version::parse(cleaned) {
        return Some(version);
    }
    match cleaned.matches('.').count() {
        0 => Version::parse(&format!("{cleaned}.0.0")).ok(),
        1 => Version::parse(&format!("{cleaned}.0")).ok(),
        _ => None,
    }
}

fn clean_version(raw: &str) -> String {
    raw.trim()
        .trim_start_matches(|c: char| c == 'v' || c == 'V')
        .to_string()
}

fn clip(text: &str) -> String {
    text.trim().chars().take(MAX_NOTES).collect()
}

fn text_of(value: &Value, keys: &[&str]) -> String {
    keys.iter()
        .filter_map(|key| value.get(*key).and_then(Value::as_str))
        .map(str::trim)
        .find(|text| !text.is_empty())
        .unwrap_or("")
        .to_string()
}

fn is_exe_asset(name: &str) -> bool {
    let name = name.to_lowercase();
    name.ends_with(".exe") && !name.contains(".old.") && !name.contains(".new.")
}

/// Формат GitHub Releases API (`/releases/latest`).
fn from_github(value: &Value) -> Option<ReleaseInfo> {
    let tag = value.get("tag_name")?.as_str()?.trim().to_string();
    if value.get("draft").and_then(Value::as_bool) == Some(true) {
        return None;
    }
    let asset = value
        .get("assets")
        .and_then(Value::as_array)
        .and_then(|assets| {
            assets
                .iter()
                .find(|asset| is_exe_asset(asset["name"].as_str().unwrap_or("")))
        });
    let (url, size, sha256) = match asset {
        Some(asset) => (
            asset["browser_download_url"].as_str().unwrap_or("").to_string(),
            asset["size"].as_u64().unwrap_or(0),
            asset["digest"]
                .as_str()
                .and_then(|digest| digest.strip_prefix("sha256:"))
                .unwrap_or("")
                .to_lowercase(),
        ),
        None => (String::new(), 0, String::new()),
    };
    Some(ReleaseInfo {
        version: clean_version(&tag),
        tag,
        name: text_of(value, &["name"]),
        date: text_of(value, &["published_at", "created_at"]),
        notes: clip(&text_of(value, &["body"])),
        url,
        size,
        sha256,
        source: String::new(),
    })
}

/// Власний формат `update.json` для дзеркал:
/// `{"version": "0.3.0", "date": "...", "notes": "...", "url": "https://...exe", "sha256": "...", "size": 123}`.
fn from_update_json(value: &Value) -> Option<ReleaseInfo> {
    let version = clean_version(value.get("version")?.as_str()?);
    let tag = text_of(value, &["tag"]);
    Some(ReleaseInfo {
        tag: if tag.is_empty() {
            format!("v{version}")
        } else {
            tag
        },
        version,
        name: text_of(value, &["name", "title"]),
        date: text_of(value, &["date", "published_at", "pub_date"]),
        notes: clip(&text_of(value, &["notes", "body", "changelog"])),
        url: text_of(value, &["url", "download_url"]),
        size: value.get("size").and_then(Value::as_u64).unwrap_or(0),
        sha256: text_of(value, &["sha256"]).to_lowercase(),
        source: String::new(),
    })
}

fn parse_release(text: &str) -> std::result::Result<ReleaseInfo, String> {
    let value: Value = serde_json::from_str(text).map_err(|e| e.to_string())?;
    let mut info = from_github(&value)
        .or_else(|| from_update_json(&value))
        .ok_or_else(|| "unknown update format".to_string())?;
    if parse_version(&info.version).is_none() {
        return Err(format!("bad version: {}", info.version));
    }
    if !info.url.is_empty() && !info.url.starts_with("https://") {
        info.url.clear();
    }
    if info.sha256.len() != 64 || !info.sha256.chars().all(|c| c.is_ascii_hexdigit()) {
        info.sha256.clear();
    }
    Ok(info)
}

/// Остання версія лаунчера: перше джерело з UPDATE_MIRRORS, що відповіло коректно.
pub async fn fetch_latest(client: &Client) -> Result<ReleaseInfo> {
    let urls = mirrors::update_candidates();
    let request = Request {
        timeout: Duration::from_secs(15),
        bust: false,
        accept: Some("application/vnd.github+json"),
    };
    match mirrors::fetch_list(client, &urls, &request, parse_release).await {
        Ok((mut info, source)) => {
            info.source = source;
            Ok(info)
        }
        Err(error) => Err(error.into_plain("launcher update")),
    }
}

pub fn is_newer(info: &ReleaseInfo) -> bool {
    match (parse_version(&info.version), parse_version(current_version())) {
        (Some(latest), Some(current)) => latest > current,
        _ => false,
    }
}

fn is_pe_file(path: &Path) -> bool {
    std::fs::File::open(path)
        .and_then(|mut file| {
            let mut header = [0u8; 2];
            std::io::Read::read_exact(&mut file, &mut header)?;
            Ok(header)
        })
        .map(|header| &header == b"MZ")
        .unwrap_or(false)
}

/// Завантажує й підміняє .exe, після чого перезапускає лаунчер.
pub async fn install(app: &AppHandle, client: &Client, info: &ReleaseInfo) -> Result<UpdateOutcome> {
    if !is_newer(info) {
        return Ok(UpdateOutcome::UpToDate);
    }
    if info.url.is_empty() {
        return fail(tr(
            "У релізі немає .exe файлу — завантаж оновлення вручну з GitHub",
            "The release has no .exe file — download the update manually from GitHub",
        ));
    }

    status(
        app,
        &t!(
            "Завантаження оновлення лаунчера {}…",
            "Downloading launcher update {}…",
            info.version
        ),
    );

    let exe = std::env::current_exe().ctx("не вдалося визначити шлях до .exe")?;
    let new_exe = exe_sibling(&exe, "new");
    let old_exe = exe_sibling(&exe, "old");

    let checksum = if info.sha256.is_empty() {
        Checksum::None
    } else {
        Checksum::Sha256(info.sha256.clone())
    };

    let file_name = info.url.rsplit('/').next().unwrap_or("launcher.exe").to_string();
    let progress = Arc::new(Progress::new(app, "launcher-update", info.size.max(1)));
    progress.set_file(&file_name);
    let reporter = progress.clone();
    download_file(client, &info.url, &new_exe, &checksum, &move |n| reporter.add(n)).await?;
    progress.finish();

    if !is_pe_file(&new_exe) {
        let _ = std::fs::remove_file(&new_exe);
        return fail("завантажений файл оновлення не є виконуваним .exe");
    }

    if old_exe.exists() {
        let _ = std::fs::remove_file(&old_exe);
    }
    std::fs::rename(&exe, &old_exe).ctx("не вдалося перейменувати поточний .exe")?;
    if let Err(error) = std::fs::rename(&new_exe, &exe) {
        let _ = std::fs::rename(&old_exe, &exe);
        return fail(format!("не вдалося підкласти новий .exe: {error}"));
    }

    status(app, tr("Перезапуск лаунчера…", "Restarting launcher…"));
    std::process::Command::new(&exe)
        .spawn()
        .ctx("не вдалося перезапустити лаунчер")?;
    app.exit(0);
    Ok(UpdateOutcome::Restarting)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_github_release() {
        let text = r###"{"tag_name":"v0.3.0","name":"Big one","body":"## New\n- a","published_at":"2026-11-20T10:00:00Z",
            "assets":[{"name":"launcher.exe","browser_download_url":"https://github.com/o/r/releases/download/v0.3.0/launcher.exe","size":10,"digest":"sha256:ABCDEFabcdef0123456789abcdef0123456789abcdef0123456789abcdef0123"}]}"###;
        let info = parse_release(text).unwrap();
        assert_eq!(info.version, "0.3.0");
        assert_eq!(info.notes, "## New\n- a");
        assert_eq!(info.sha256.len(), 64);
        assert!(info.url.ends_with("launcher.exe"));
    }

    #[test]
    fn parses_update_json() {
        let text = r#"{"version":"0.4","notes":"fixes","url":"https://mirror.example/launcher.exe","date":"2026-12-01"}"#;
        let info = parse_release(text).unwrap();
        assert_eq!(info.version, "0.4");
        assert_eq!(info.tag, "v0.4");
        assert!(parse_release(r#"{"hello":"world"}"#).is_err());
    }
}
