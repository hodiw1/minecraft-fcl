use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use reqwest::Client;
use semver::Version;
use serde::Deserialize;
use tauri::AppHandle;

use crate::constants::releases_api_url;
use crate::error::{fail, Context, Result};
use crate::events::{status, Progress};
use crate::util::{download_file, Checksum};

#[derive(Deserialize)]
struct Release {
    tag_name: String,
    #[serde(default)]
    assets: Vec<ReleaseAsset>,
}

#[derive(Deserialize)]
struct ReleaseAsset {
    name: String,
    browser_download_url: String,
    #[serde(default)]
    size: u64,
    #[serde(default)]
    digest: Option<String>,
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

fn parse_version(raw: &str) -> Option<Version> {
    let cleaned = raw.trim().trim_start_matches(|c: char| c == 'v' || c == 'V');
    if let Ok(version) = Version::parse(cleaned) {
        return Some(version);
    }
    match cleaned.matches('.').count() {
        0 => Version::parse(&format!("{cleaned}.0.0")).ok(),
        1 => Version::parse(&format!("{cleaned}.0")).ok(),
        _ => None,
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

pub async fn check_and_update(app: &AppHandle, client: &Client) -> Result<UpdateOutcome> {
    let release = client
        .get(releases_api_url())
        .header("Accept", "application/vnd.github+json")
        .send()
        .await?
        .error_for_status()?
        .json::<Release>()
        .await?;

    let latest = parse_version(&release.tag_name).ctx("неправильний тег версії на GitHub")?;
    let current = parse_version(current_version()).ctx("неправильна версія в Cargo.toml")?;
    if latest <= current {
        return Ok(UpdateOutcome::UpToDate);
    }

    let asset = release
        .assets
        .iter()
        .find(|a| {
            let name = a.name.to_lowercase();
            name.ends_with(".exe") && !name.contains(".old.") && !name.contains(".new.")
        })
        .ctx("у релізі немає .exe файлу")?;

    status(app, &format!("Завантаження оновлення лаунчера {latest}…"));

    let exe = std::env::current_exe().ctx("не вдалося визначити шлях до .exe")?;
    let new_exe = exe_sibling(&exe, "new");
    let old_exe = exe_sibling(&exe, "old");

    let checksum = match asset
        .digest
        .as_deref()
        .and_then(|d| d.strip_prefix("sha256:"))
    {
        Some(hash) => Checksum::Sha256(hash.to_string()),
        None => Checksum::None,
    };

    let progress = Arc::new(Progress::new(app, "launcher-update", asset.size));
    progress.set_file(&asset.name);
    let reporter = progress.clone();
    download_file(
        client,
        &asset.browser_download_url,
        &new_exe,
        &checksum,
        &move |n| reporter.add(n),
    )
    .await?;
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

    status(app, "Перезапуск лаунчера…");
    std::process::Command::new(&exe)
        .spawn()
        .ctx("не вдалося перезапустити лаунчер")?;
    app.exit(0);
    Ok(UpdateOutcome::Restarting)
}
