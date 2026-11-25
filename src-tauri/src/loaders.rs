use std::path::Path;
use std::process::Stdio;

use reqwest::Client;
use serde_json::Value;
use tauri::AppHandle;
use tokio::process::Command;

use crate::error::{fail, Context, Result};
use crate::events::status;
use crate::manifest::Manifest;
use crate::paths::Paths;
use crate::t;
use crate::util::{download_file, hide_window, noop, read_zip_text, Checksum};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LoaderKind {
    Vanilla,
    Fabric,
    Forge,
    NeoForge,
}

impl LoaderKind {
    pub fn parse(raw: &str) -> Result<Self> {
        match raw.trim().to_lowercase().as_str() {
            "" | "vanilla" => Ok(LoaderKind::Vanilla),
            "fabric" => Ok(LoaderKind::Fabric),
            "forge" => Ok(LoaderKind::Forge),
            "neoforge" | "neo-forge" | "neo_forge" => Ok(LoaderKind::NeoForge),
            other => fail(format!("невідомий завантажувач у маніфесті: {other}")),
        }
    }
}

pub async fn prepare_loader(
    app: &AppHandle,
    client: &Client,
    paths: &Paths,
    manifest: &Manifest,
    java: &Path,
) -> Result<String> {
    let kind = LoaderKind::parse(&manifest.loader.kind)?;
    let mc = manifest.minecraft_version.trim();
    let loader_version = manifest.loader.version.trim();

    if kind == LoaderKind::Vanilla {
        return Ok(mc.to_string());
    }
    if loader_version.is_empty() {
        return fail("у маніфесті не вказано версію завантажувача");
    }

    match kind {
        LoaderKind::Fabric => prepare_fabric(app, client, paths, mc, loader_version).await,
        _ => prepare_forge_like(app, client, paths, kind, mc, loader_version, java).await,
    }
}

async fn prepare_fabric(
    app: &AppHandle,
    client: &Client,
    paths: &Paths,
    mc: &str,
    loader_version: &str,
) -> Result<String> {
    let id = format!("fabric-loader-{loader_version}-{mc}");
    let json_path = paths.version_json(&id);
    if !json_path.is_file() {
        status(
            app,
            &t!(
                "Встановлення Fabric {}…",
                "Installing Fabric {}…",
                loader_version
            ),
        );
        let url = format!(
            "https://meta.fabricmc.net/v2/versions/loader/{mc}/{loader_version}/profile/json"
        );
        download_file(client, &url, &json_path, &Checksum::None, &noop).await?;
    }
    Ok(id)
}

fn installer_url(kind: LoaderKind, mc: &str, loader_version: &str) -> String {
    match kind {
        LoaderKind::NeoForge => {
            if mc == "1.20.1" {
                format!(
                    "https://maven.neoforged.net/releases/net/neoforged/forge/{mc}-{loader_version}/forge-{mc}-{loader_version}-installer.jar"
                )
            } else {
                format!(
                    "https://maven.neoforged.net/releases/net/neoforged/neoforge/{loader_version}/neoforge-{loader_version}-installer.jar"
                )
            }
        }
        _ => format!(
            "https://maven.minecraftforge.net/net/minecraftforge/forge/{mc}-{loader_version}/forge-{mc}-{loader_version}-installer.jar"
        ),
    }
}

fn installer_version_id(installer: &Path) -> Result<String> {
    if let Some(text) = read_zip_text(installer, "version.json")? {
        let value: Value = serde_json::from_str(&text)?;
        if let Some(id) = value.get("id").and_then(|v| v.as_str()) {
            return Ok(id.to_string());
        }
    }
    if let Some(text) = read_zip_text(installer, "install_profile.json")? {
        let value: Value = serde_json::from_str(&text)?;
        if let Some(id) = value.pointer("/versionInfo/id").and_then(|v| v.as_str()) {
            return Ok(id.to_string());
        }
    }
    fail("не вдалося визначити ідентифікатор версії в інсталяторі")
}

fn tail(text: &str, limit: usize) -> String {
    let chars: Vec<char> = text.chars().collect();
    let start = chars.len().saturating_sub(limit);
    chars[start..].iter().collect()
}

async fn prepare_forge_like(
    app: &AppHandle,
    client: &Client,
    paths: &Paths,
    kind: LoaderKind,
    mc: &str,
    loader_version: &str,
    java: &Path,
) -> Result<String> {
    let label = if kind == LoaderKind::NeoForge {
        "neoforge"
    } else {
        "forge"
    };
    let marker = paths
        .cache
        .join(format!("loader-{label}-{mc}-{loader_version}.id"));
    if let Ok(saved) = tokio::fs::read_to_string(&marker).await {
        let id = saved.trim().to_string();
        if !id.is_empty() && paths.version_json(&id).is_file() {
            status(app, &t!("{} вже встановлено", "{} is already installed", id));
            return Ok(id);
        }
    }

    tokio::fs::create_dir_all(&paths.cache).await?;
    let installer = paths
        .cache
        .join(format!("{label}-{mc}-{loader_version}-installer.jar"));
    if !installer.is_file() {
        status(
            app,
            &t!(
                "Завантаження інсталятора {}…",
                "Downloading {} installer…",
                label
            ),
        );
        let url = installer_url(kind, mc, loader_version);
        download_file(client, &url, &installer, &Checksum::None, &noop).await?;
    }

    let profiles = paths.mc.join("launcher_profiles.json");
    if !profiles.exists() {
        tokio::fs::write(&profiles, r#"{"profiles":{},"settings":{},"version":3}"#).await?;
    }

    status(
        app,
        &t!(
            "Встановлення {} (це може тривати кілька хвилин)…",
            "Installing {} (this may take a few minutes)…",
            label
        ),
    );
    let mut command = Command::new(java);
    command
        .arg("-jar")
        .arg(&installer)
        .arg("--installClient")
        .arg(&paths.mc)
        .current_dir(&paths.mc)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    hide_window(&mut command);
    let output = command
        .output()
        .await
        .ctx("не вдалося запустити інсталятор")?;
    if !output.status.success() {
        let _ = tokio::fs::remove_file(&installer).await;
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        return fail(format!(
            "інсталятор {label} завершився з помилкою: {}",
            tail(&format!("{stdout}\n{stderr}"), 900)
        ));
    }

    let id = installer_version_id(&installer)?;
    if !paths.version_json(&id).is_file() {
        return fail(format!("інсталятор не створив версію {id}"));
    }
    tokio::fs::write(&marker, &id).await?;
    Ok(id)
}
