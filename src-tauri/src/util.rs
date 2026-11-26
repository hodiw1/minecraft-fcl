use std::path::{Path, PathBuf};
use std::time::Duration;

use futures_util::StreamExt;
use reqwest::Client;
use serde::de::DeserializeOwned;
use sha1::Sha1;
use sha2::{Digest, Sha256, Sha512};
use tokio::io::AsyncWriteExt;

use crate::constants::LAUNCHER_NAME;
use crate::error::{fail, AppError, Result};
use crate::mirrors;

#[derive(Clone, Debug)]
pub enum Checksum {
    None,
    Sha1(String),
    Sha256(String),
    Sha512(String),
}

impl Checksum {
    pub fn expected(&self) -> Option<&str> {
        match self {
            Checksum::None => None,
            Checksum::Sha1(value) | Checksum::Sha256(value) | Checksum::Sha512(value) => {
                Some(value.as_str())
            }
        }
    }
}

enum Hasher {
    None,
    Sha1(Sha1),
    Sha256(Sha256),
    Sha512(Sha512),
}

impl Hasher {
    fn new(checksum: &Checksum) -> Self {
        match checksum {
            Checksum::None => Hasher::None,
            Checksum::Sha1(_) => Hasher::Sha1(Sha1::new()),
            Checksum::Sha256(_) => Hasher::Sha256(Sha256::new()),
            Checksum::Sha512(_) => Hasher::Sha512(Sha512::new()),
        }
    }

    fn update(&mut self, data: &[u8]) {
        match self {
            Hasher::None => {}
            Hasher::Sha1(hasher) => hasher.update(data),
            Hasher::Sha256(hasher) => hasher.update(data),
            Hasher::Sha512(hasher) => hasher.update(data),
        }
    }

    fn finish(self) -> Option<String> {
        match self {
            Hasher::None => None,
            Hasher::Sha1(hasher) => Some(hex::encode(hasher.finalize())),
            Hasher::Sha256(hasher) => Some(hex::encode(hasher.finalize())),
            Hasher::Sha512(hasher) => Some(hex::encode(hasher.finalize())),
        }
    }
}

pub fn noop(_: u64) {}

pub fn http_client() -> Result<Client> {
    let client = Client::builder()
        .user_agent(format!("{}/{}", LAUNCHER_NAME, env!("CARGO_PKG_VERSION")))
        // Заблокований хост часто «висить» без відповіді — не чекаємо його довго,
        // а переходимо до наступного дзеркала.
        .connect_timeout(Duration::from_secs(10))
        // Потік, що завис на пів дорозі, теж вважається збоєм дзеркала.
        .read_timeout(Duration::from_secs(45))
        .build()?;
    Ok(client)
}

/// JSON за посиланням; для файлів із GitHub-репозиторіїв — з перебором дзеркал.
pub async fn fetch_json<T: DeserializeOwned>(client: &Client, url: &str) -> Result<T> {
    mirrors::get_json(client, url, false)
        .await
        .map_err(|error| error.into_plain(url))
}

pub fn hide_window(command: &mut tokio::process::Command) {
    #[cfg(windows)]
    {
        command.creation_flags(0x0800_0000);
    }
    #[cfg(not(windows))]
    {
        let _ = command;
    }
}

pub fn join_rel(base: &Path, relative: &str) -> PathBuf {
    let mut out = base.to_path_buf();
    for part in relative.split('/') {
        if part.is_empty()
            || part == "."
            || part == ".."
            || part.contains('\\')
            || part.contains(':')
        {
            continue;
        }
        out.push(part);
    }
    out
}

pub fn hash_file(path: &Path, checksum: &Checksum) -> Result<Option<String>> {
    let mut hasher = Hasher::new(checksum);
    let mut file = std::fs::File::open(path)?;
    let mut buffer = vec![0u8; 65536];
    loop {
        let read = std::io::Read::read(&mut file, &mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hasher.finish())
}

/// SHA-256 файлу у нижньому регістрі.
pub fn sha256_file(path: &Path) -> Result<String> {
    Ok(hash_file(path, &Checksum::Sha256(String::new()))?.unwrap_or_default())
}

pub fn verify_file(path: &Path, checksum: &Checksum) -> bool {
    let Some(expected) = checksum.expected() else {
        return path.is_file();
    };
    match hash_file(path, checksum) {
        Ok(Some(actual)) => actual.eq_ignore_ascii_case(expected),
        _ => false,
    }
}

pub async fn verify_file_async(path: PathBuf, checksum: Checksum) -> bool {
    tokio::task::spawn_blocking(move || verify_file(&path, &checksum))
        .await
        .unwrap_or(false)
}

fn part_path(dest: &Path) -> PathBuf {
    let mut name = dest.as_os_str().to_os_string();
    name.push(".part");
    PathBuf::from(name)
}

async fn try_download(
    client: &Client,
    url: &str,
    dest: &Path,
    checksum: &Checksum,
    on_bytes: &(dyn Fn(u64) + Send + Sync),
) -> Result<()> {
    if let Some(parent) = dest.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    let temp = part_path(dest);
    let response = client.get(url).send().await.map_err(|error| {
        if mirrors::is_network(&error) {
            mirrors::mark_down(url);
        }
        AppError::from(error)
    })?;
    let status = response.status();
    if !status.is_success() {
        if mirrors::is_host_failure(status) {
            mirrors::mark_down(url);
        }
        return fail(format!("HTTP {status}: {url}"));
    }
    let mut stream = response.bytes_stream();
    let mut file = tokio::fs::File::create(&temp).await?;
    let mut hasher = Hasher::new(checksum);
    while let Some(chunk) = stream.next().await {
        let chunk = match chunk {
            Ok(chunk) => chunk,
            Err(error) => {
                mirrors::mark_down(url);
                drop(file);
                let _ = tokio::fs::remove_file(&temp).await;
                return Err(error.into());
            }
        };
        hasher.update(&chunk);
        file.write_all(&chunk).await?;
        on_bytes(chunk.len() as u64);
    }
    file.flush().await?;
    drop(file);
    if let (Some(expected), Some(actual)) = (checksum.expected(), hasher.finish()) {
        if !actual.eq_ignore_ascii_case(expected) {
            let _ = tokio::fs::remove_file(&temp).await;
            return fail("контрольна сума не збігається");
        }
    }
    tokio::fs::rename(&temp, dest).await?;
    Ok(())
}

fn is_disk_full(message: &str) -> bool {
    let lower = message.to_lowercase();
    lower.contains("os error 112")
        || lower.contains("not enough space")
        || lower.contains("no space left")
}

/// Завантажує файл із перевіркою контрольної суми. Для файлів із GitHub-репозиторіїв
/// (і будь-яких посилань, якщо в конфігу є проксі `{url}`) автоматично перебирає дзеркала:
/// мережева помилка, таймаут, 403/404/429/451/5xx чи битий файл → наступне дзеркало,
/// включно з підміною домену для посилань із manifest.json.
pub async fn download_file(
    client: &Client,
    url: &str,
    dest: &Path,
    checksum: &Checksum,
    on_bytes: &(dyn Fn(u64) + Send + Sync),
) -> Result<()> {
    let candidates = mirrors::content_candidates(url);
    let rounds: u64 = if candidates.len() > 1 { 2 } else { 3 };
    let mut last_error = String::new();
    for round in 1..=rounds {
        for candidate in &candidates {
            match try_download(client, candidate, dest, checksum, on_bytes).await {
                Ok(()) => {
                    mirrors::mark_up(candidate);
                    return Ok(());
                }
                Err(error) => {
                    // Диск переповнений — інші дзеркала не допоможуть.
                    if is_disk_full(&error.0) {
                        return Err(error);
                    }
                    last_error = error.0;
                }
            }
        }
        if round < rounds {
            tokio::time::sleep(Duration::from_millis(500 * round)).await;
        }
    }
    fail(format!("не вдалося завантажити {url}: {last_error}"))
}

pub fn extract_zip(zip_path: &Path, dest: &Path, exclude_prefixes: &[String]) -> Result<()> {
    let file = std::fs::File::open(zip_path)?;
    let mut archive = zip::ZipArchive::new(file)?;
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index)?;
        let Some(relative) = entry.enclosed_name() else {
            continue;
        };
        let name = entry.name().to_string();
        if exclude_prefixes
            .iter()
            .any(|prefix| name.starts_with(prefix.as_str()))
        {
            continue;
        }
        let out_path = dest.join(&relative);
        if entry.is_dir() {
            std::fs::create_dir_all(&out_path)?;
            continue;
        }
        if let Some(parent) = out_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut out_file = std::fs::File::create(&out_path)?;
        std::io::copy(&mut entry, &mut out_file)?;
    }
    Ok(())
}

pub fn read_zip_text(zip_path: &Path, name: &str) -> Result<Option<String>> {
    let file = std::fs::File::open(zip_path)?;
    let mut archive = zip::ZipArchive::new(file)?;
    let mut entry = match archive.by_name(name) {
        Ok(entry) => entry,
        Err(zip::result::ZipError::FileNotFound) => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    let mut text = String::new();
    std::io::Read::read_to_string(&mut entry, &mut text)?;
    Ok(Some(text))
}
