use std::path::{Path, PathBuf};
use std::sync::Arc;

use reqwest::Client;
use tauri::AppHandle;

use crate::error::{fail, Result};
use crate::events::{status, Progress};
use crate::i18n::tr;
use crate::paths::Paths;
use crate::t;
use crate::util::{download_file, extract_zip, Checksum};

const JAVA_BIN: &str = if cfg!(windows) { "java.exe" } else { "java" };

pub fn adoptium_major(requested: u32) -> u32 {
    match requested {
        0..=8 => 8,
        9..=11 => 11,
        12..=17 => 17,
        18..=21 => 21,
        22..=25 => 25,
        other => other,
    }
}

async fn probe_size(client: &Client, url: &str) -> Option<u64> {
    let response = client.head(url).send().await.ok()?;
    response
        .headers()
        .get(reqwest::header::CONTENT_LENGTH)?
        .to_str()
        .ok()?
        .parse::<u64>()
        .ok()
        .filter(|value| *value > 0)
}

fn find_java_root(dir: &Path) -> Result<Option<PathBuf>> {
    if dir.join("bin").join(JAVA_BIN).is_file() {
        return Ok(Some(dir.to_path_buf()));
    }
    for item in std::fs::read_dir(dir)? {
        let path = item?.path();
        if path.is_dir() && path.join("bin").join(JAVA_BIN).is_file() {
            return Ok(Some(path));
        }
    }
    Ok(None)
}

pub async fn ensure_java(
    app: &AppHandle,
    client: &Client,
    paths: &Paths,
    requested: u32,
) -> Result<PathBuf> {
    let major = adoptium_major(requested);
    let home = paths.runtime.join(format!("java-{major}"));
    let exe = home.join("bin").join(JAVA_BIN);
    if exe.is_file() {
        status(
            app,
            &t!("Java {} вже встановлена", "Java {} is already installed", major),
        );
        return Ok(exe);
    }

    tokio::fs::create_dir_all(&paths.runtime).await?;
    tokio::fs::create_dir_all(&paths.cache).await?;

    status(
        app,
        &t!(
            "Завантаження Java {} (Temurin JRE)…",
            "Downloading Java {} (Temurin JRE)…",
            major
        ),
    );
    let arch = if cfg!(target_arch = "aarch64") {
        "aarch64"
    } else {
        "x64"
    };
    let url = format!(
        "https://api.adoptium.net/v3/binary/latest/{major}/ga/windows/{arch}/jre/hotspot/normal/eclipse"
    );
    let size = probe_size(client, &url).await.unwrap_or(45_000_000);
    let zip_path = paths.cache.join(format!("java-{major}.zip"));

    let progress = Arc::new(Progress::new(app, "java", size));
    progress.set_file(&format!("Temurin JRE {major}"));
    let reporter = progress.clone();
    download_file(client, &url, &zip_path, &Checksum::None, &move |n| {
        reporter.add(n)
    })
    .await?;
    progress.finish();

    status(app, tr("Розпакування Java…", "Unpacking Java…"));
    let temp = paths.runtime.join(format!("java-{major}.tmp"));
    if temp.exists() {
        tokio::fs::remove_dir_all(&temp).await?;
    }
    tokio::fs::create_dir_all(&temp).await?;

    let zip_clone = zip_path.clone();
    let temp_clone = temp.clone();
    tokio::task::spawn_blocking(move || extract_zip(&zip_clone, &temp_clone, &[])).await??;

    let temp_for_search = temp.clone();
    let root = tokio::task::spawn_blocking(move || find_java_root(&temp_for_search)).await??;
    let Some(root) = root else {
        let _ = tokio::fs::remove_dir_all(&temp).await;
        return fail("у завантаженому архіві Java не знайдено java.exe");
    };

    if home.exists() {
        tokio::fs::remove_dir_all(&home).await?;
    }
    tokio::fs::rename(&root, &home).await?;
    let _ = tokio::fs::remove_dir_all(&temp).await;
    let _ = tokio::fs::remove_file(&zip_path).await;

    if !exe.is_file() {
        return fail("Java встановлено некоректно");
    }
    Ok(exe)
}
