//! Оновлення лаунчера: інформація про реліз (версія, дата, повний опис змін) з
//! [`UPDATE_MIRRORS`](crate::constants::UPDATE_MIRRORS) і окреме встановлення.
//!
//! Два режими:
//! - **встановлена версія** (поруч з .exe лежить `uninstall.exe` NSIS-інсталятора) —
//!   завантажуємо `terrapolis-installer.exe` (або `*-setup.exe`) у тимчасову папку й запускаємо його звичайним викликом;
//!   інсталятор сам замінює файли й перезапускає лаунчер (`/R`);
//! - **портативна версія** — завантажуємо новий .exe поруч і міняємо файли перейменуванням
//!   (без .bat / PowerShell-скриптів).

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
    /// Посилання на портативний `terrapolis-portable.exe` (оригінальне; дзеркала підставляються під час завантаження).
    pub url: String,
    pub size: u64,
    pub sha256: String,
    /// Посилання на NSIS-інсталятор `terrapolis-installer.exe` (для встановленої версії), може бути порожнім.
    pub setup_url: String,
    pub setup_size: u64,
    pub setup_sha256: String,
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

/// Тимчасова папка для завантаженого інсталятора оновлення.
pub fn setup_dir() -> PathBuf {
    std::env::temp_dir().join("terrapolis-update")
}

/// Лаунчер встановлено NSIS-інсталятором (Tauri кладе `uninstall.exe` поруч із головним .exe).
fn is_installed(exe: &Path) -> bool {
    exe.parent()
        .map(|dir| dir.join("uninstall.exe").is_file())
        .unwrap_or(false)
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
        // Інсталятор попереднього оновлення вже відпрацював — прибираємо його з %TEMP%.
        let setup = setup_dir();
        if setup.is_dir() {
            let _ = std::fs::remove_dir_all(setup);
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

/// Імена файлів у GitHub Release (їх задає `.github/workflows/release.yml`).
const INSTALLER_ASSET: &str = "terrapolis-installer.exe";
const PORTABLE_ASSET: &str = "terrapolis-portable.exe";

/// Пріоритет файлу як інсталятора (менше — краще): `terrapolis-installer.exe`,
/// потім стандартна назва NSIS від Tauri (`Terrapolis_0.4.0_x64-setup.exe`) для сумісності.
fn installer_rank(name: &str) -> Option<u8> {
    let name = name.trim().to_lowercase();
    if name == INSTALLER_ASSET {
        Some(0)
    } else if name.ends_with("-setup.exe") || name.ends_with("_setup.exe") {
        Some(1)
    } else {
        None
    }
}

/// Пріоритет файлу як портативного .exe: `terrapolis-portable.exe`, потім старі назви
/// `Terrapolis.exe` і `launcher.exe` (релізи до переходу на інсталятор).
fn portable_rank(name: &str) -> Option<u8> {
    match name.trim().to_lowercase().as_str() {
        PORTABLE_ASSET => Some(0),
        "terrapolis.exe" => Some(1),
        "launcher.exe" => Some(2),
        _ => None,
    }
}

/// (url, size, sha256) файлу релізу з найкращим пріоритетом.
fn github_asset(value: &Value, rank: fn(&str) -> Option<u8>) -> (String, u64, String) {
    let asset = value
        .get("assets")
        .and_then(Value::as_array)
        .and_then(|assets| {
            assets
                .iter()
                .filter_map(|asset| rank(asset["name"].as_str().unwrap_or("")).map(|r| (r, asset)))
                .min_by_key(|(r, _)| *r)
                .map(|(_, asset)| asset)
        });
    match asset {
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
    }
}

/// Формат GitHub Releases API (`/releases/latest`).
fn from_github(value: &Value) -> Option<ReleaseInfo> {
    let tag = value.get("tag_name")?.as_str()?.trim().to_string();
    if value.get("draft").and_then(Value::as_bool) == Some(true) {
        return None;
    }
    let (url, size, sha256) = github_asset(value, portable_rank);
    let (setup_url, setup_size, setup_sha256) = github_asset(value, installer_rank);
    Some(ReleaseInfo {
        version: clean_version(&tag),
        tag,
        name: text_of(value, &["name"]),
        date: text_of(value, &["published_at", "created_at"]),
        notes: clip(&text_of(value, &["body"])),
        url,
        size,
        sha256,
        setup_url,
        setup_size,
        setup_sha256,
        source: String::new(),
    })
}

/// Власний формат `update.json` для дзеркал:
/// `{"version": "0.3.0", "date": "...", "notes": "...", "url": "https://...exe", "sha256": "...", "size": 123,
///   "setup_url": "https://...-setup.exe", "setup_sha256": "...", "setup_size": 123}`.
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
        setup_url: text_of(value, &["setup_url", "installer_url"]),
        setup_size: value.get("setup_size").and_then(Value::as_u64).unwrap_or(0),
        setup_sha256: text_of(value, &["setup_sha256"]).to_lowercase(),
        source: String::new(),
    })
}

fn is_sha256(text: &str) -> bool {
    text.len() == 64 && text.chars().all(|c| c.is_ascii_hexdigit())
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
    if !info.setup_url.is_empty() && !info.setup_url.starts_with("https://") {
        info.setup_url.clear();
    }
    if !is_sha256(&info.sha256) {
        info.sha256.clear();
    }
    if !is_sha256(&info.setup_sha256) {
        info.setup_sha256.clear();
    }
    Ok(info)
}

/// Остання версія лаунчера: перше джерело з UPDATE_MIRRORS, що відповіло коректно.
/// `fresh = true` (кнопка «Синхронізувати») додає `?t=<час>`, щоб обійти кеш CDN дзеркал.
pub async fn fetch_latest(client: &Client, fresh: bool) -> Result<ReleaseInfo> {
    let urls = mirrors::update_candidates();
    let request = Request {
        timeout: Duration::from_secs(15),
        bust: fresh,
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

/// Завантажує файл оновлення з прогресом у сповіщенні й перевіряє, що це справжній .exe.
async fn download_update(
    app: &AppHandle,
    client: &Client,
    url: &str,
    size: u64,
    sha256: &str,
    target: &Path,
) -> Result<()> {
    let checksum = if sha256.is_empty() {
        Checksum::None
    } else {
        Checksum::Sha256(sha256.to_string())
    };
    let file_name = url.rsplit('/').next().unwrap_or("launcher.exe").to_string();
    let progress = Arc::new(Progress::new(app, "launcher-update", size.max(1)));
    progress.set_file(&file_name);
    let reporter = progress.clone();
    download_file(client, url, target, &checksum, &move |n| reporter.add(n)).await?;
    progress.finish();

    if !is_pe_file(target) {
        let _ = std::fs::remove_file(target);
        return fail("завантажений файл оновлення не є виконуваним .exe");
    }
    Ok(())
}

/// Безпечна назва файлу інсталятора в %TEMP% (з URL беремо лише останню частину шляху).
fn setup_file_name(url: &str, version: &str) -> String {
    let last = url
        .split(['?', '#'])
        .next()
        .unwrap_or("")
        .rsplit('/')
        .next()
        .unwrap_or("");
    let clean: String = last
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
        .collect();
    if installer_rank(&clean).is_some() && !clean.starts_with('.') {
        clean
    } else {
        let version: String = version
            .chars()
            .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-'))
            .collect();
        format!("Terrapolis_{version}_x64-setup.exe")
    }
}

/// Встановлює оновлення: інсталятором (встановлена версія) або заміною .exe (портативна).
pub async fn install(app: &AppHandle, client: &Client, info: &ReleaseInfo) -> Result<UpdateOutcome> {
    if !is_newer(info) {
        return Ok(UpdateOutcome::UpToDate);
    }
    let exe = std::env::current_exe().ctx("не вдалося визначити шлях до .exe")?;
    if is_installed(&exe) && !info.setup_url.is_empty() {
        return install_setup(app, client, info).await;
    }
    // Портативний режим (або реліз без інсталятора — тоді встановлена копія в папці
    // користувача оновлюється так само, заміною .exe).
    if info.url.is_empty() {
        return fail(tr(
            "У релізі немає .exe файлу — завантаж оновлення вручну з GitHub",
            "The release has no .exe file — download the update manually from GitHub",
        ));
    }
    install_portable(app, client, info, &exe).await
}

/// Встановлена версія: `terrapolis-installer.exe` → %TEMP%\terrapolis-update\ → звичайний запуск інсталятора.
/// Активний .exe лаунчер не чіпає — файли замінює сам інсталятор після нашого виходу.
async fn install_setup(app: &AppHandle, client: &Client, info: &ReleaseInfo) -> Result<UpdateOutcome> {
    status(
        app,
        &t!(
            "Завантаження інсталятора оновлення {}…",
            "Downloading the update installer {}…",
            info.version
        ),
    );
    let dir = setup_dir();
    std::fs::create_dir_all(&dir).ctx("не вдалося створити тимчасову папку для оновлення")?;
    let setup = dir.join(setup_file_name(&info.setup_url, &info.version));
    download_update(
        app,
        client,
        &info.setup_url,
        info.setup_size,
        &info.setup_sha256,
        &setup,
    )
    .await?;

    status(
        app,
        tr("Запуск інсталятора оновлення…", "Starting the update installer…"),
    );
    // Прапорці NSIS-інсталятора Tauri: /P — пасивний режим (лише смуга прогресу),
    // /R — запустити лаунчер після встановлення, /UPDATE — режим оновлення.
    std::process::Command::new(&setup)
        .args(["/P", "/R", "/UPDATE"])
        .spawn()
        .ctx(tr(
            "не вдалося запустити інсталятор оновлення",
            "failed to start the update installer",
        ))?;
    app.exit(0);
    Ok(UpdateOutcome::Restarting)
}

/// Портативна версія: новий .exe поруч, заміна перейменуванням і перезапуск.
async fn install_portable(
    app: &AppHandle,
    client: &Client,
    info: &ReleaseInfo,
    exe: &Path,
) -> Result<UpdateOutcome> {
    status(
        app,
        &t!(
            "Завантаження оновлення лаунчера {}…",
            "Downloading launcher update {}…",
            info.version
        ),
    );

    let new_exe = exe_sibling(exe, "new");
    let old_exe = exe_sibling(exe, "old");
    download_update(app, client, &info.url, info.size, &info.sha256, &new_exe).await?;

    if old_exe.exists() {
        let _ = std::fs::remove_file(&old_exe);
    }
    std::fs::rename(exe, &old_exe).ctx("не вдалося перейменувати поточний .exe")?;
    if let Err(error) = std::fs::rename(&new_exe, exe) {
        let _ = std::fs::rename(&old_exe, exe);
        return fail(format!("не вдалося підкласти новий .exe: {error}"));
    }

    status(app, tr("Перезапуск лаунчера…", "Restarting launcher…"));
    std::process::Command::new(exe)
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

    #[test]
    fn picks_named_installer_and_portable_assets() {
        // Порядок файлів у релізі навмисно «незручний»: запасні назви стоять першими.
        let text = r###"{"tag_name":"v0.4.0","assets":[
            {"name":"Terrapolis_0.4.0_x64-setup.exe","browser_download_url":"https://github.com/o/r/releases/download/v0.4.0/Terrapolis_0.4.0_x64-setup.exe","size":30},
            {"name":"launcher.exe","browser_download_url":"https://github.com/o/r/releases/download/v0.4.0/launcher.exe","size":5},
            {"name":"terrapolis-installer.exe","browser_download_url":"https://github.com/o/r/releases/download/v0.4.0/terrapolis-installer.exe","size":20},
            {"name":"terrapolis-portable.exe","browser_download_url":"https://github.com/o/r/releases/download/v0.4.0/terrapolis-portable.exe","size":10}]}"###;
        let info = parse_release(text).unwrap();
        assert!(info.url.ends_with("/terrapolis-portable.exe"));
        assert_eq!(info.size, 10);
        assert!(info.setup_url.ends_with("/terrapolis-installer.exe"));
        assert_eq!(info.setup_size, 20);
        assert_eq!(
            setup_file_name(&info.setup_url, &info.version),
            "terrapolis-installer.exe"
        );
        // Інсталятор ніколи не вважається портативною версією і навпаки.
        assert_eq!(portable_rank("terrapolis-installer.exe"), None);
        assert_eq!(installer_rank("terrapolis-portable.exe"), None);
    }

    #[test]
    fn falls_back_to_legacy_asset_names() {
        let text = r###"{"tag_name":"v0.4.0","assets":[
            {"name":"launcher.exe","browser_download_url":"https://github.com/o/r/releases/download/v0.4.0/launcher.exe","size":5},
            {"name":"Terrapolis.exe","browser_download_url":"https://github.com/o/r/releases/download/v0.4.0/Terrapolis.exe","size":10},
            {"name":"Terrapolis_0.4.0_x64-setup.exe","browser_download_url":"https://github.com/o/r/releases/download/v0.4.0/Terrapolis_0.4.0_x64-setup.exe","size":20}]}"###;
        let info = parse_release(text).unwrap();
        assert!(info.url.ends_with("/Terrapolis.exe"));
        assert!(info.setup_url.ends_with("-setup.exe"));
        assert_eq!(
            setup_file_name(&info.setup_url, &info.version),
            "Terrapolis_0.4.0_x64-setup.exe"
        );
        assert_eq!(
            setup_file_name("https://x.example/dl?id=1", "0.4.0"),
            "Terrapolis_0.4.0_x64-setup.exe"
        );
    }
}
