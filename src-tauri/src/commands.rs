use serde::{Deserialize, Serialize};
use tauri::{AppHandle, State};

use crate::config::{
    self, new_id, validate_nickname, Config, Profile, ProjectBinding, GAME_LANGS, PRESETS, UI_LANGS,
};
use crate::error::{fail, AppError, Context, Result};
use crate::events::{phase, status};
use crate::i18n::{self, tr};
use crate::instance::{self, DiskInstance};
use crate::launch::{self, LastExit, LaunchRequest};
use crate::manifest::{self, Manifest};
use crate::paths::{Paths, DATA_CHILDREN};
use crate::project::{self, InstanceEntry};
use crate::server_status::{ping as ping_server, ServerStatus};
use crate::state::AppState;
use crate::sync::{self, SyncReport};
use crate::system;
use crate::t;
use crate::updater::{self, UpdateOutcome};

const MAX_PROFILES: usize = 20;
const MAX_SKIN_BYTES: usize = 1024 * 1024;

// ───────────────────────────── допоміжне ─────────────────────────────

async fn load_ctx(app: &AppHandle) -> Result<(Config, Paths)> {
    let root = Paths::app_root(app)?;
    let config = config::load(&root).await;
    i18n::set_lang(&config.ui_lang);
    let paths = Paths::resolve(app, &config)?;
    Ok((config, paths))
}

/// Атомарно змінює config.json (без гонок між командами).
async fn update_config<R>(
    app: &AppHandle,
    state: &AppState,
    change: impl FnOnce(&mut Config) -> Result<R>,
) -> Result<R> {
    let _lock = state.config_lock.lock().await;
    let root = Paths::app_root(app)?;
    let mut config = config::load(&root).await;
    let out = change(&mut config)?;
    config::save(&root, &config).await?;
    Ok(out)
}

fn busy_error() -> AppError {
    AppError(
        tr(
            "Зачекай — лаунчер ще виконує попередню дію",
            "Please wait — the launcher is still busy",
        )
        .to_string(),
    )
}

fn running_error() -> AppError {
    AppError(
        tr(
            "Спершу закрий Minecraft — поки гра запущена, це зробити не можна",
            "Close Minecraft first — this can't be done while the game is running",
        )
        .to_string(),
    )
}

fn check_id(id: &str) -> Result<()> {
    if project::is_safe_id(id) {
        Ok(())
    } else {
        fail(tr("Некоректна назва збірки", "Invalid modpack id"))
    }
}

fn require_project(config: &Config) -> Result<ProjectBinding> {
    config.project.clone().ctx(tr(
        "Проєкт не підключено — введи код проєкту",
        "No project connected — enter a project code",
    ))
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

/// Безпечний максимум: уся пам'ять ПК мінус 2 ГБ для Windows і програм.
fn max_ram_mb(total: u64) -> u32 {
    total.saturating_sub(2048).clamp(1024, 65536) as u32
}

fn clamp_ram(value: u32) -> u32 {
    let max = max_ram_mb(total_ram_mb());
    let value = value.clamp(1024, max);
    value - value % 256
}

// ───────────────────────────── моделі для UI ─────────────────────────────

#[derive(Serialize)]
pub struct SettingsView {
    pub ram_mb: u32,
    pub graphics_preset: String,
    pub ui_lang: String,
    pub game_lang: String,
    pub data_dir: String,
    pub default_data_dir: String,
}

#[derive(Serialize)]
pub struct InitInfo {
    pub launcher_version: String,
    pub total_ram_mb: u64,
    pub max_ram_mb: u32,
    pub settings: SettingsView,
    pub profiles: Vec<Profile>,
    pub active_profile: String,
    pub project: Option<ProjectBinding>,
    pub active_instance: String,
    pub game_running: Option<String>,
}

#[derive(Serialize)]
pub struct ProfilesView {
    pub profiles: Vec<Profile>,
    pub active_profile: String,
}

impl ProfilesView {
    fn of(config: &Config) -> Self {
        Self {
            profiles: config.profiles.clone(),
            active_profile: config.active_profile.clone(),
        }
    }
}

#[derive(Serialize)]
pub struct InstanceView {
    pub id: String,
    pub name: String,
    pub description: String,
    pub server: String,
    pub image: String,
    pub minecraft_version: String,
    pub loader: String,
    pub loader_version: String,
    pub recommended_ram_mb: Option<u32>,
    pub size_bytes: Option<u64>,
    pub installed: bool,
    pub local_version: String,
    pub remote_version: String,
    pub update_available: bool,
}

#[derive(Serialize)]
pub struct ProjectView {
    pub code: String,
    pub name: String,
    pub description: String,
    pub offline: bool,
    pub active_instance: String,
    pub instances: Vec<InstanceView>,
}

#[derive(Serialize)]
pub struct PlayOutcome {
    pub offline: bool,
    pub first_install: bool,
}

#[derive(Serialize)]
pub struct UpdateCheck {
    pub restarting: bool,
    pub error: Option<String>,
}

#[derive(Deserialize, Default)]
pub struct SettingsPatch {
    #[serde(default)]
    pub ram_mb: Option<u32>,
    #[serde(default)]
    pub graphics_preset: Option<String>,
    #[serde(default)]
    pub ui_lang: Option<String>,
    #[serde(default)]
    pub game_lang: Option<String>,
    #[serde(default)]
    pub active_instance: Option<String>,
}

async fn build_view(
    paths: &Paths,
    binding: &ProjectBinding,
    entry: &InstanceEntry,
) -> InstanceView {
    let state = instance::load_state(paths, &entry.id).await;
    let remote =
        manifest::load_cached(&project::remote_manifest_cache(paths, binding, &entry.id)).await;
    let installed = state.installed && paths.instance_dir(&entry.id).is_dir();

    let (remote_version, manifest_size, mc, loader, loader_version, ram) = match &remote {
        Some(m) => (
            m.version.clone(),
            Some(manifest::total_size(m)),
            m.minecraft_version.clone(),
            m.loader.kind.clone(),
            m.loader.version.clone(),
            m.recommended_ram_mb,
        ),
        None => (
            String::new(),
            None,
            String::new(),
            String::new(),
            String::new(),
            None,
        ),
    };
    let size_bytes = entry.size_mb.map(|mb| mb * 1024 * 1024).or(manifest_size);
    let update_available =
        installed && !remote_version.is_empty() && remote_version != state.pack_version;

    InstanceView {
        id: entry.id.clone(),
        name: entry.name.clone(),
        description: entry.description.clone(),
        server: entry.server.clone(),
        image: entry.image.clone(),
        minecraft_version: if entry.minecraft_version.is_empty() {
            mc
        } else {
            entry.minecraft_version.clone()
        },
        loader: if entry.loader.is_empty() {
            loader
        } else {
            entry.loader.clone()
        },
        loader_version,
        recommended_ram_mb: entry.recommended_ram_mb.or(ram),
        size_bytes,
        installed,
        local_version: state.pack_version,
        remote_version,
        update_available,
    }
}

// ───────────────────────────── старт ─────────────────────────────

#[tauri::command]
pub async fn get_init_info(app: AppHandle, state: State<'_, AppState>) -> Result<InitInfo> {
    let (config, paths) = load_ctx(&app).await?;
    let total = total_ram_mb();
    let max = max_ram_mb(total);
    Ok(InitInfo {
        launcher_version: updater::current_version().to_string(),
        total_ram_mb: total,
        max_ram_mb: max,
        settings: SettingsView {
            ram_mb: config.ram_mb.clamp(1024, max),
            graphics_preset: config.graphics_preset.clone(),
            ui_lang: config.ui_lang.clone(),
            game_lang: config.game_lang.clone(),
            data_dir: paths.data.to_string_lossy().to_string(),
            default_data_dir: paths.root.to_string_lossy().to_string(),
        },
        profiles: config.profiles.clone(),
        active_profile: config.active_profile.clone(),
        project: config.project.clone(),
        active_instance: config.active_instance.clone(),
        game_running: state.running_game(),
    })
}

#[tauri::command]
pub async fn check_launcher_update(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<UpdateCheck> {
    let Ok(_guard) = state.busy.try_lock() else {
        return Ok(UpdateCheck {
            restarting: false,
            error: None,
        });
    };
    load_ctx(&app).await?;
    status(
        &app,
        tr(
            "Перевірка оновлень лаунчера…",
            "Checking for launcher updates…",
        ),
    );
    match updater::check_and_update(&app, &state.client).await {
        Ok(UpdateOutcome::Restarting) => Ok(UpdateCheck {
            restarting: true,
            error: None,
        }),
        Ok(UpdateOutcome::UpToDate) => Ok(UpdateCheck {
            restarting: false,
            error: None,
        }),
        Err(error) => Ok(UpdateCheck {
            restarting: false,
            error: Some(error.0),
        }),
    }
}

// ───────────────────────────── проєкт ─────────────────────────────

#[tauri::command]
pub async fn connect_project(
    app: AppHandle,
    state: State<'_, AppState>,
    code: String,
) -> Result<ProjectBinding> {
    let (_, paths) = load_ctx(&app).await?;
    let (binding, index) = project::resolve_code(&state.client, &code).await?;
    let _ = project::save_cache(&paths, &binding, &index).await;
    let saved = binding.clone();
    update_config(&app, &state, move |config| {
        let keep = project::find(&index, &config.active_instance).is_some();
        if !keep {
            config.active_instance = index
                .instances
                .first()
                .map(|e| e.id.clone())
                .unwrap_or_default();
        }
        config.project = Some(saved);
        Ok(())
    })
    .await?;
    Ok(binding)
}

#[tauri::command]
pub async fn leave_project(app: AppHandle, state: State<'_, AppState>) -> Result<()> {
    update_config(&app, &state, |config| {
        config.project = None;
        config.active_instance.clear();
        Ok(())
    })
    .await
}

#[tauri::command]
pub async fn list_projects(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Vec<project::ProjectCard>> {
    let (_, paths) = load_ctx(&app).await?;
    project::discover_projects(&state.client, &paths).await
}

#[tauri::command]
pub async fn load_project(app: AppHandle, state: State<'_, AppState>) -> Result<ProjectView> {
    let (config, paths) = load_ctx(&app).await?;
    let binding = require_project(&config)?;
    let (index, offline) = project::load_index(&state.client, &paths, &binding).await?;

    let mut active = config.active_instance.clone();
    if project::find(&index, &active).is_none() {
        active = index
            .instances
            .first()
            .map(|e| e.id.clone())
            .unwrap_or_default();
        let chosen = active.clone();
        update_config(&app, &state, move |config| {
            config.active_instance = chosen;
            Ok(())
        })
        .await?;
    }

    let mut instances = Vec::with_capacity(index.instances.len());
    for entry in &index.instances {
        instances.push(build_view(&paths, &binding, entry).await);
    }
    let name = if index.name.trim().is_empty() {
        binding.name.clone()
    } else {
        index.name.clone()
    };
    Ok(ProjectView {
        code: binding.code.clone(),
        name,
        description: index.description.clone(),
        offline,
        active_instance: active,
        instances,
    })
}

/// Підтягує manifest.json збірки, щоб показати точний розмір і чи є оновлення.
#[tauri::command]
pub async fn instance_details(
    app: AppHandle,
    state: State<'_, AppState>,
    instance_id: String,
) -> Result<InstanceView> {
    check_id(&instance_id)?;
    let (config, paths) = load_ctx(&app).await?;
    let binding = require_project(&config)?;
    let index = match project::load_cache(&paths, &binding).await {
        Some(index) => index,
        None => {
            project::load_index(&state.client, &paths, &binding)
                .await?
                .0
        }
    };
    let entry = project::find(&index, &instance_id).cloned().ctx(tr(
        "Збірку не знайдено в проєкті",
        "Modpack not found in the project",
    ))?;
    if let Ok(fresh) = manifest::fetch(&state.client, &entry.manifest).await {
        let _ = manifest::save_cache(
            &project::remote_manifest_cache(&paths, &binding, &entry.id),
            &fresh,
        )
        .await;
    }
    Ok(build_view(&paths, &binding, &entry).await)
}

#[tauri::command]
pub async fn server_status(address: String) -> Result<ServerStatus> {
    Ok(ping_server(&address).await)
}

// ───────────────────────────── налаштування ─────────────────────────────

#[tauri::command]
pub async fn save_settings(
    app: AppHandle,
    state: State<'_, AppState>,
    patch: SettingsPatch,
) -> Result<()> {
    if let Some(preset) = &patch.graphics_preset {
        if !PRESETS.contains(&preset.as_str()) {
            return fail("unknown preset");
        }
    }
    if let Some(lang) = &patch.ui_lang {
        if !UI_LANGS.contains(&lang.as_str()) {
            return fail("unknown ui language");
        }
    }
    if let Some(lang) = &patch.game_lang {
        if !GAME_LANGS.contains(&lang.as_str()) {
            return fail("unknown game language");
        }
    }
    if let Some(id) = &patch.active_instance {
        check_id(id)?;
    }

    let game_lang_changed = update_config(&app, &state, |config| {
        if let Some(ram) = patch.ram_mb {
            config.ram_mb = clamp_ram(ram);
        }
        if let Some(preset) = patch.graphics_preset.clone() {
            config.graphics_preset = preset;
        }
        if let Some(lang) = patch.ui_lang.clone() {
            i18n::set_lang(&lang);
            config.ui_lang = lang;
        }
        if let Some(id) = patch.active_instance.clone() {
            config.active_instance = id;
        }
        let mut changed = None;
        if let Some(lang) = patch.game_lang.clone() {
            if lang != config.game_lang {
                changed = Some(lang.clone());
            }
            config.game_lang = lang;
        }
        Ok(changed)
    })
    .await?;

    // Мову гри одразу прописуємо у всі встановлені збірки (лише ключ lang, решта options.txt не змінюється).
    if let Some(lang) = game_lang_changed {
        let (_, paths) = load_ctx(&app).await?;
        for disk in instance::list_on_disk(&paths).await {
            if disk.installed {
                let _ = instance::apply_lang(&paths, &disk.id, &lang).await;
            }
        }
    }
    Ok(())
}

#[tauri::command]
pub async fn apply_graphics_preset(
    app: AppHandle,
    state: State<'_, AppState>,
    instance_id: String,
) -> Result<()> {
    check_id(&instance_id)?;
    if state.running_game().as_deref() == Some(instance_id.as_str()) {
        return Err(running_error());
    }
    let (config, paths) = load_ctx(&app).await?;
    let mut st = instance::load_state(&paths, &instance_id).await;
    if !st.installed {
        return fail(tr(
            "Збірку ще не встановлено — пресет застосується автоматично під час встановлення",
            "The modpack isn't installed yet — the preset will be applied during installation",
        ));
    }
    instance::apply_preset(&paths, &instance_id, &config.graphics_preset).await?;
    st.preset_applied = true;
    instance::save_state(&paths, &instance_id, &st).await
}

#[tauri::command]
pub async fn pick_folder() -> Result<Option<String>> {
    system::pick_folder(tr(
        "Обери папку для лаунчера та збірок",
        "Choose a folder for the launcher and modpacks",
    ))
    .await
}

#[tauri::command]
pub async fn set_data_dir(
    app: AppHandle,
    state: State<'_, AppState>,
    path: String,
    move_files: bool,
) -> Result<String> {
    let _guard = state.busy.try_lock().map_err(|_| busy_error())?;
    if state.running_game().is_some() {
        return Err(running_error());
    }
    let (_, paths) = load_ctx(&app).await?;
    let target = system::normalize_dir(&path)?;
    let old = paths.data.clone();
    if target == old {
        return Ok(old.to_string_lossy().to_string());
    }
    for child in DATA_CHILDREN {
        if target.starts_with(old.join(child)) {
            return fail(tr(
                "Нова папка не може бути всередині поточної папки зі збірками",
                "The new folder can't be inside the current modpack folder",
            ));
        }
    }
    let check = target.clone();
    tokio::task::spawn_blocking(move || system::check_writable(&check)).await??;

    if move_files {
        for child in DATA_CHILDREN {
            let destination = target.join(child);
            if destination.exists() && !system::is_dir_empty(&destination) {
                return fail(t!(
                    "У новій папці вже є дані лаунчера («{}»). Обери порожню папку.",
                    "The new folder already contains launcher data (\"{}\"). Choose an empty folder.",
                    child
                ));
            }
        }
        for child in DATA_CHILDREN {
            status(&app, &t!("Перенесення «{}»…", "Moving \"{}\"…", child));
            let from = old.join(child);
            let to = target.join(child);
            tokio::task::spawn_blocking(move || system::move_dir(&from, &to)).await??;
        }
        status(&app, tr("Файли перенесено", "Files moved"));
    }

    let root = paths.root.clone();
    let stored = if target == root {
        String::new()
    } else {
        target.to_string_lossy().to_string()
    };
    update_config(&app, &state, move |config| {
        config.data_dir = stored;
        Ok(())
    })
    .await?;
    Ok(target.to_string_lossy().to_string())
}

#[tauri::command]
pub async fn open_data_folder(app: AppHandle) -> Result<()> {
    let (_, paths) = load_ctx(&app).await?;
    system::open_folder(&paths.data)
}

// ───────────────────────────── профілі та скіни ─────────────────────────────

#[tauri::command]
pub async fn add_profile(
    app: AppHandle,
    state: State<'_, AppState>,
    nickname: String,
) -> Result<ProfilesView> {
    load_ctx(&app).await?;
    let nickname = nickname.trim().to_string();
    validate_nickname(&nickname)?;
    update_config(&app, &state, move |config| {
        if config
            .profiles
            .iter()
            .any(|p| p.nickname.eq_ignore_ascii_case(&nickname))
        {
            return fail(tr(
                "Такий нікнейм уже є у списку",
                "This nickname is already in the list",
            ));
        }
        if config.profiles.len() >= MAX_PROFILES {
            return fail(tr(
                "Забагато профілів (максимум 20)",
                "Too many profiles (max 20)",
            ));
        }
        let profile = Profile {
            id: new_id(),
            nickname,
            has_skin: false,
        };
        config.active_profile = profile.id.clone();
        config.profiles.push(profile);
        Ok(ProfilesView::of(config))
    })
    .await
}

#[tauri::command]
pub async fn remove_profile(
    app: AppHandle,
    state: State<'_, AppState>,
    profile_id: String,
) -> Result<ProfilesView> {
    let (_, paths) = load_ctx(&app).await?;
    let view = update_config(&app, &state, |config| {
        if config.profiles.len() <= 1 {
            return fail(tr(
                "Має залишитися хоча б один профіль",
                "At least one profile must remain",
            ));
        }
        config.profiles.retain(|p| p.id != profile_id);
        if !config
            .profiles
            .iter()
            .any(|p| p.id == config.active_profile)
        {
            config.active_profile = config
                .profiles
                .first()
                .map(|p| p.id.clone())
                .unwrap_or_default();
        }
        Ok(ProfilesView::of(config))
    })
    .await?;
    let _ = tokio::fs::remove_file(paths.skin_file(&profile_id)).await;
    Ok(view)
}

#[tauri::command]
pub async fn set_active_profile(
    app: AppHandle,
    state: State<'_, AppState>,
    profile_id: String,
) -> Result<ProfilesView> {
    update_config(&app, &state, |config| {
        if !config.profiles.iter().any(|p| p.id == profile_id) {
            return fail(tr("Профіль не знайдено", "Profile not found"));
        }
        config.active_profile = profile_id;
        Ok(ProfilesView::of(config))
    })
    .await
}

fn png_size(bytes: &[u8]) -> Option<(u32, u32)> {
    const SIGNATURE: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a];
    if bytes.len() < 24 || bytes[..8] != SIGNATURE || &bytes[12..16] != b"IHDR" {
        return None;
    }
    let width = u32::from_be_bytes([bytes[16], bytes[17], bytes[18], bytes[19]]);
    let height = u32::from_be_bytes([bytes[20], bytes[21], bytes[22], bytes[23]]);
    Some((width, height))
}

#[tauri::command]
pub async fn save_skin(
    app: AppHandle,
    state: State<'_, AppState>,
    profile_id: String,
    bytes: Vec<u8>,
) -> Result<ProfilesView> {
    let (_, paths) = load_ctx(&app).await?;
    if bytes.len() > MAX_SKIN_BYTES {
        return fail(tr("Файл скіна завеликий", "The skin file is too large"));
    }
    let valid =
        matches!(png_size(&bytes), Some((w, h)) if matches!(w, 64 | 128) && (h == w || h == w / 2));
    if !valid {
        return fail(tr(
            "Це не схоже на скін Minecraft: потрібен .png розміром 64×64 (або 64×32)",
            "This doesn't look like a Minecraft skin: a 64×64 (or 64×32) .png is required",
        ));
    }
    let exists = config::load(&paths.root)
        .await
        .profiles
        .iter()
        .any(|p| p.id == profile_id);
    if !exists {
        return fail(tr("Профіль не знайдено", "Profile not found"));
    }
    tokio::fs::create_dir_all(paths.skins()).await?;
    tokio::fs::write(paths.skin_file(&profile_id), &bytes).await?;
    update_config(&app, &state, |config| {
        if let Some(profile) = config.profiles.iter_mut().find(|p| p.id == profile_id) {
            profile.has_skin = true;
        }
        Ok(ProfilesView::of(config))
    })
    .await
}

#[tauri::command]
pub async fn clear_skin(
    app: AppHandle,
    state: State<'_, AppState>,
    profile_id: String,
) -> Result<ProfilesView> {
    let (_, paths) = load_ctx(&app).await?;
    let _ = tokio::fs::remove_file(paths.skin_file(&profile_id)).await;
    update_config(&app, &state, |config| {
        if let Some(profile) = config.profiles.iter_mut().find(|p| p.id == profile_id) {
            profile.has_skin = false;
        }
        Ok(ProfilesView::of(config))
    })
    .await
}

#[tauri::command]
pub async fn get_skin(app: AppHandle, profile_id: String) -> Result<Option<Vec<u8>>> {
    let (_, paths) = load_ctx(&app).await?;
    Ok(tokio::fs::read(paths.skin_file(&profile_id)).await.ok())
}

// ───────────────────────────── збірки ─────────────────────────────

async fn fetch_manifest_for(
    app: &AppHandle,
    state: &AppState,
    paths: &Paths,
    binding: &ProjectBinding,
    instance_id: &str,
) -> Result<(InstanceEntry, Option<Manifest>)> {
    let (index, _) = project::load_index(&state.client, paths, binding).await?;
    let entry = project::find(&index, instance_id).cloned().ctx(tr(
        "Збірку не знайдено в проєкті",
        "Modpack not found in the project",
    ))?;
    status(
        app,
        tr(
            "Перевірка оновлень збірки…",
            "Checking for modpack updates…",
        ),
    );
    match manifest::fetch(&state.client, &entry.manifest).await {
        Ok(fresh) => {
            let _ = manifest::save_cache(
                &project::remote_manifest_cache(paths, binding, &entry.id),
                &fresh,
            )
            .await;
            Ok((entry, Some(fresh)))
        }
        Err(_) => Ok((entry, None)),
    }
}

#[tauri::command]
pub async fn play(
    app: AppHandle,
    state: State<'_, AppState>,
    instance_id: String,
) -> Result<PlayOutcome> {
    check_id(&instance_id)?;
    let _guard = state.busy.try_lock().map_err(|_| busy_error())?;
    if state.running_game().is_some() {
        return fail(tr(
            "Гра вже запущена. Закрий її, щоб запустити знову.",
            "The game is already running. Close it to start again.",
        ));
    }
    let (config, paths) = load_ctx(&app).await?;
    let profile = config.active_profile().cloned().ctx(tr(
        "Спершу додай нікнейм у вкладці «Профілі»",
        "Add a nickname in the Profiles tab first",
    ))?;
    validate_nickname(&profile.nickname)?;
    let binding = require_project(&config)?;
    paths.ensure_shared_dirs().await?;
    phase(&app, "checking");

    let mut st = instance::load_state(&paths, &instance_id).await;
    let first_install = !st.installed;
    let (entry, fresh) = fetch_manifest_for(&app, &state, &paths, &binding, &instance_id).await?;

    st.name = entry.name.clone();
    st.project = binding.code.clone();
    instance::ensure_dirs(&paths, &instance_id).await?;
    instance::save_state(&paths, &instance_id, &st).await?;

    let installed_copy = instance::manifest_cache(&paths, &instance_id);
    let (manifest, offline) = match fresh {
        Some(fresh) => {
            sync::sync_instance(&app, &state.client, &paths, &instance_id, &fresh, false).await?;
            manifest::save_cache(&installed_copy, &fresh).await?;
            (fresh, false)
        }
        None => {
            let cached = if st.installed {
                manifest::load_cached(&installed_copy).await
            } else {
                None
            };
            match cached {
                Some(cached) => {
                    status(
                        &app,
                        tr(
                            "Немає інтернету — запуск без перевірки оновлень",
                            "No internet — starting without checking for updates",
                        ),
                    );
                    (cached, true)
                }
                None => {
                    return fail(tr(
                        "Для встановлення збірки потрібен інтернет. Перевір підключення і спробуй ще раз.",
                        "Installing a modpack requires internet. Check your connection and try again.",
                    ))
                }
            }
        }
    };

    let game_dir = paths.instance_dir(&instance_id);
    // Пресет графіки й мову гри пишемо лише один раз — коли options.txt ще не існує.
    // Далі гравець сам керує налаштуваннями в грі, лаунчер їх не перезаписує.
    if !game_dir.join("options.txt").exists() {
        instance::apply_preset(&paths, &instance_id, &config.graphics_preset).await?;
        instance::apply_lang(&paths, &instance_id, &config.game_lang).await?;
        let mut fresh_state = instance::load_state(&paths, &instance_id).await;
        fresh_state.preset_applied = true;
        instance::save_state(&paths, &instance_id, &fresh_state).await?;
    }

    launch::place_skin(
        &paths,
        &game_dir,
        &profile.id,
        &profile.nickname,
        profile.has_skin,
    )
    .await;

    let ram = clamp_ram(config.ram_mb);
    let prepared =
        launch::prepare(&app, &state.client, &paths, &manifest, &game_dir, false).await?;
    launch::spawn(
        &app,
        &paths,
        state.game.clone(),
        &prepared,
        &LaunchRequest {
            instance_id: &instance_id,
            nickname: &profile.nickname,
            ram_mb: ram,
        },
    )?;
    Ok(PlayOutcome {
        offline,
        first_install,
    })
}

/// "Ремонт": повна перевірка SHA-256, докачування пошкодженого, видалення зайвих .jar.
/// Налаштування гравця, світи, скріншоти й мітки карт не чіпаються.
#[tauri::command]
pub async fn repair_instance(
    app: AppHandle,
    state: State<'_, AppState>,
    instance_id: String,
) -> Result<SyncReport> {
    check_id(&instance_id)?;
    let _guard = state.busy.try_lock().map_err(|_| busy_error())?;
    if state.running_game().is_some() {
        return Err(running_error());
    }
    let (config, paths) = load_ctx(&app).await?;
    let binding = require_project(&config)?;
    phase(&app, "checking");
    let (_, fresh) = fetch_manifest_for(&app, &state, &paths, &binding, &instance_id).await?;
    let manifest = fresh.ctx(tr(
        "Для перевірки файлів потрібен інтернет",
        "Checking files requires an internet connection",
    ))?;
    let report =
        sync::sync_instance(&app, &state.client, &paths, &instance_id, &manifest, true).await?;
    manifest::save_cache(&instance::manifest_cache(&paths, &instance_id), &manifest).await?;
    let game_dir = paths.instance_dir(&instance_id);
    launch::prepare(&app, &state.client, &paths, &manifest, &game_dir, true).await?;
    status(&app, tr("Перевірку завершено", "Check complete"));
    Ok(report)
}

#[tauri::command]
pub async fn open_instance_folder(app: AppHandle, instance_id: String) -> Result<()> {
    check_id(&instance_id)?;
    let (_, paths) = load_ctx(&app).await?;
    system::open_folder(&paths.instance_dir(&instance_id))
}

#[tauri::command]
pub async fn list_installed(app: AppHandle) -> Result<Vec<DiskInstance>> {
    let (_, paths) = load_ctx(&app).await?;
    Ok(instance::list_on_disk(&paths).await)
}

#[tauri::command]
pub async fn delete_instance(
    app: AppHandle,
    state: State<'_, AppState>,
    instance_id: String,
) -> Result<()> {
    check_id(&instance_id)?;
    let _guard = state.busy.try_lock().map_err(|_| busy_error())?;
    if state.running_game().as_deref() == Some(instance_id.as_str()) {
        return Err(running_error());
    }
    let (_, paths) = load_ctx(&app).await?;
    instance::delete(&paths, &instance_id).await
}

#[tauri::command]
pub async fn delete_all_instances(app: AppHandle, state: State<'_, AppState>) -> Result<usize> {
    let _guard = state.busy.try_lock().map_err(|_| busy_error())?;
    if state.running_game().is_some() {
        return Err(running_error());
    }
    let (_, paths) = load_ctx(&app).await?;
    let mut removed = 0;
    for disk in instance::list_on_disk(&paths).await {
        if project::is_safe_id(&disk.id) {
            instance::delete(&paths, &disk.id).await?;
            removed += 1;
        }
    }
    Ok(removed)
}

/// Текст для кнопки "Скопіювати звіт для адміністратора".
#[tauri::command]
pub async fn crash_report(app: AppHandle, instance_id: String) -> Result<String> {
    check_id(&instance_id)?;
    let (config, paths) = load_ctx(&app).await?;
    let st = instance::load_state(&paths, &instance_id).await;
    let game_dir = paths.instance_dir(&instance_id);
    let manifest = manifest::load_cached(&instance::manifest_cache(&paths, &instance_id)).await;
    let last: LastExit =
        tokio::fs::read_to_string(paths.instance_meta(&instance_id).join("last_exit.json"))
            .await
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default();
    let log_path = instance::game_log(&paths, &instance_id);

    let mut out = String::new();
    out.push_str("=== Launcher report ===\n");
    out.push_str(&format!("Launcher: v{}\n", updater::current_version()));
    if let Some(project) = &config.project {
        out.push_str(&format!("Project: {} ({})\n", project.name, project.code));
    }
    out.push_str(&format!(
        "Modpack: {} [{}] v{}\n",
        st.name, instance_id, st.pack_version
    ));
    if let Some(m) = &manifest {
        out.push_str(&format!(
            "Minecraft: {} • {} {}\n",
            m.minecraft_version, m.loader.kind, m.loader.version
        ));
    }
    let profile = config
        .active_profile()
        .map(|p| p.nickname.clone())
        .unwrap_or_default();
    out.push_str(&format!("Nickname: {profile}\n"));
    out.push_str(&format!(
        "RAM: {} MB (PC total {} MB) • Java: {}\n",
        last.ram_mb,
        total_ram_mb(),
        last.java
    ));
    out.push_str(&format!(
        "OS: {} {} • Exit code: {}\n",
        std::env::consts::OS,
        std::env::consts::ARCH,
        last.code
            .map(|c| c.to_string())
            .unwrap_or_else(|| "-".into())
    ));
    if let Some(hint) = &last.hint {
        out.push_str(&format!("Diagnosis: {hint}\n"));
    }

    let report_text = tokio::task::spawn_blocking(move || {
        let mut text = String::new();
        if let Some(report) = launch::latest_crash_report(&game_dir) {
            let name = report
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();
            text.push_str(&format!("\n--- crash-reports/{name} ---\n"));
            text.push_str(&instance::last_lines(
                &instance::tail_text(&report, 48 * 1024),
                120,
            ));
            text.push('\n');
        }
        text.push_str("\n--- game_output.log (last 150 lines) ---\n");
        text.push_str(&instance::last_lines(
            &instance::tail_text(&log_path, 96 * 1024),
            150,
        ));
        text
    })
    .await?;
    out.push_str(&report_text);
    Ok(out)
}
