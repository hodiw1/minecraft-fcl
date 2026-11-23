use serde::Serialize;
use tauri::{AppHandle, State};

use crate::config::{self, validate_nickname, Config};
use crate::error::{Context, Result};
use crate::events::status;
use crate::launch;
use crate::manifest::{self, Manifest};
use crate::paths::Paths;
use crate::state::AppState;
use crate::sync;
use crate::updater::{self, UpdateOutcome};

#[derive(Serialize)]
pub struct PackInfo {
    pub name: String,
    pub version: String,
    pub minecraft_version: String,
    pub loader: String,
    pub loader_version: String,
    pub recommended_ram_mb: Option<u32>,
}

impl PackInfo {
    fn from_manifest(manifest: &Manifest) -> Self {
        let loader = if manifest.loader.kind.trim().is_empty() {
            "vanilla".to_string()
        } else {
            manifest.loader.kind.clone()
        };
        Self {
            name: manifest.name.clone(),
            version: manifest.version.clone(),
            minecraft_version: manifest.minecraft_version.clone(),
            loader,
            loader_version: manifest.loader.version.clone(),
            recommended_ram_mb: manifest.recommended_ram_mb,
        }
    }
}

#[derive(Serialize)]
pub struct InitInfo {
    pub launcher_version: String,
    pub nickname: String,
    pub ram_mb: u32,
    pub total_ram_mb: u64,
    pub max_ram_mb: u32,
    pub pack: Option<PackInfo>,
}

#[derive(Serialize)]
pub struct BootstrapResult {
    pub restarting: bool,
    pub offline: bool,
    pub pack: Option<PackInfo>,
}

fn total_ram_mb() -> u64 {
    let mut system = sysinfo::System::new();
    system.refresh_memory();
    let total = system.total_memory() / 1024 / 1024;
    if total == 0 {
        8192
    } else {
        total
    }
}

fn max_ram_mb(total: u64) -> u32 {
    total.saturating_sub(1024).clamp(2048, 32768) as u32
}

#[tauri::command]
pub async fn get_init_info(app: AppHandle) -> Result<InitInfo> {
    let paths = Paths::new(&app)?;
    let config = config::load(&paths).await;
    let total = total_ram_mb();
    let max = max_ram_mb(total);
    let pack = manifest::load_cached(&paths)
        .await
        .map(|m| PackInfo::from_manifest(&m));
    Ok(InitInfo {
        launcher_version: updater::current_version().to_string(),
        nickname: config.nickname,
        ram_mb: config.ram_mb.clamp(1024, max),
        total_ram_mb: total,
        max_ram_mb: max,
        pack,
    })
}

#[tauri::command]
pub async fn save_settings(app: AppHandle, nickname: String, ram_mb: u32) -> Result<()> {
    let nickname = nickname.trim().to_string();
    validate_nickname(&nickname)?;
    let paths = Paths::new(&app)?;
    let ram = ram_mb.clamp(1024, max_ram_mb(total_ram_mb()));
    config::save(&paths, &Config { nickname, ram_mb: ram }).await
}

#[tauri::command]
pub async fn bootstrap(app: AppHandle, state: State<'_, AppState>) -> Result<BootstrapResult> {
    let _guard = state.busy.lock().await;
    let client = state.client.clone();
    let paths = Paths::new(&app)?;
    paths.ensure_dirs().await?;

    status(&app, "Перевірка оновлень лаунчера…");
    match updater::check_and_update(&app, &client).await {
        Ok(UpdateOutcome::Restarting) => {
            return Ok(BootstrapResult {
                restarting: true,
                offline: false,
                pack: None,
            });
        }
        Ok(UpdateOutcome::UpToDate) => {}
        Err(error) => status(
            &app,
            &format!("Не вдалося перевірити оновлення лаунчера: {}", error.0),
        ),
    }

    status(&app, "Завантаження маніфесту збірки…");
    let (manifest, offline) = match manifest::fetch(&client).await {
        Ok(manifest) => {
            manifest::save_cache(&paths, &manifest).await?;
            (manifest, false)
        }
        Err(error) => match manifest::load_cached(&paths).await {
            Some(manifest) => {
                status(
                    &app,
                    &format!(
                        "Немає зв'язку з GitHub ({}). Використовується локальна збірка",
                        error.0
                    ),
                );
                (manifest, true)
            }
            None => return Err(error),
        },
    };

    if !offline {
        sync::sync_pack(&app, &client, &paths, &manifest).await?;
    }
    status(&app, "Готово до запуску");

    Ok(BootstrapResult {
        restarting: false,
        offline,
        pack: Some(PackInfo::from_manifest(&manifest)),
    })
}

#[tauri::command]
pub async fn play(
    app: AppHandle,
    state: State<'_, AppState>,
    nickname: String,
    ram_mb: u32,
) -> Result<()> {
    let _guard = state.busy.lock().await;
    let nickname = nickname.trim().to_string();
    validate_nickname(&nickname)?;

    let paths = Paths::new(&app)?;
    paths.ensure_dirs().await?;
    let ram = ram_mb.clamp(1024, max_ram_mb(total_ram_mb()));
    config::save(
        &paths,
        &Config {
            nickname: nickname.clone(),
            ram_mb: ram,
        },
    )
    .await?;

    let manifest = manifest::load_cached(&paths)
        .await
        .ctx("збірку ще не синхронізовано, перезапусти лаунчер")?;
    let client = state.client.clone();
    launch::run(&app, &client, &paths, &manifest, &nickname, ram).await
}
