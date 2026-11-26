use serde::{Deserialize, Serialize};
use tauri::{AppHandle, State, WebviewWindow};

use std::path::PathBuf;

use crate::changelog::{self, Changelog};
use crate::constants::ENABLE_CURSEFORGE;
use crate::config::{
    self, new_id, validate_api_key, validate_nickname, Config, Profile, ProjectBinding, GAME_LANGS,
    PRESETS, UI_LANGS,
};
use crate::custom::{self, CustomSpec, LoaderVersion, McVersion};
use crate::error::{fail, AppError, Context, Result};
use crate::events::{phase, status, step};
use crate::i18n::{self, tr};
use crate::import_pack::{self, ImportReport};
use crate::instance::{self, DiskInstance, DiskUsage};
use crate::launch::{self, LastExit, LaunchRequest};
use crate::loaders::LoaderKind;
use crate::manifest::{self, Manifest};
use crate::mirrors::{self, MirrorsInfo};
use crate::mods::{self, Api, Source, Target};
use crate::pack_debugger::{self, DebugReport};
use crate::paths::{Paths, DATA_CHILDREN};
use crate::project::{self, InstanceEntry};
use crate::server_status::{ping as ping_server, ServerStatus};
use crate::state::AppState;
use crate::sync::{self, SyncReport};
use crate::system;
use crate::t;
use crate::updater::{self, ReleaseInfo, UpdateOutcome};

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

/// HWND вікна лаунчера для нативних діалогів (як `isize`, щоб передати в окремий потік).
fn window_handle(window: &WebviewWindow) -> Option<isize> {
    #[cfg(windows)]
    {
        window.hwnd().ok().map(|hwnd| hwnd.0 as isize)
    }
    #[cfg(not(windows))]
    {
        let _ = window;
        None
    }
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
    pub auto_update_launcher: bool,
    /// Ключ CurseForge збережено в налаштуваннях.
    pub curseforge_key_set: bool,
    /// Маска збереженого ключа («abcd…wxyz») — сам ключ в інтерфейс не повертається.
    pub curseforge_key_hint: String,
    /// Ключ береться зі змінної середовища.
    pub curseforge_key_env: bool,
}

fn mask_key(key: &str) -> String {
    let chars: Vec<char> = key.chars().collect();
    if chars.len() <= 8 {
        return "•".repeat(chars.len().max(4));
    }
    let head: String = chars[..4].iter().collect();
    let tail: String = chars[chars.len() - 4..].iter().collect();
    format!("{head}…{tail}")
}

fn settings_view(config: &Config, paths: &Paths, max: u32) -> SettingsView {
    let saved = config.curseforge_api_key.trim();
    SettingsView {
        ram_mb: config.ram_mb.clamp(1024, max),
        graphics_preset: config.graphics_preset.clone(),
        ui_lang: config.ui_lang.clone(),
        game_lang: config.game_lang.clone(),
        data_dir: paths.data.to_string_lossy().to_string(),
        default_data_dir: paths.root.to_string_lossy().to_string(),
        auto_update_launcher: config.auto_update_launcher,
        curseforge_key_set: !saved.is_empty(),
        curseforge_key_hint: if saved.is_empty() {
            String::new()
        } else {
            mask_key(saved)
        },
        curseforge_key_env: saved.is_empty() && config.curseforge_key().is_some(),
    }
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
    /// Папка збірки в репозиторії: звідси беруться icon.png і background.png.
    pub asset_base: String,
    pub minecraft_version: String,
    pub loader: String,
    pub loader_version: String,
    pub recommended_ram_mb: Option<u32>,
    pub size_bytes: Option<u64>,
    pub installed: bool,
    pub local_version: String,
    pub remote_version: String,
    pub update_available: bool,
    /// Власна збірка гравця (не з проєкту).
    pub custom: bool,
    /// Кількість увімкнених модів (лише для власних збірок).
    pub mods_count: Option<usize>,
}

#[derive(Serialize)]
pub struct ProjectView {
    pub code: String,
    pub name: String,
    pub description: String,
    /// icon.png у корені репозиторію проєкту.
    pub icon: String,
    /// Список із кешу: жодне дзеркало не відповіло.
    pub offline: bool,
    /// Навіть кешу немає — показано лише збірки, встановлені на цьому комп'ютері.
    pub local_only: bool,
    pub active_instance: String,
    pub instances: Vec<InstanceView>,
}

#[derive(Serialize)]
pub struct PlayOutcome {
    pub offline: bool,
    pub first_install: bool,
    /// Звіт pre-launch дебагера (лише для збірок проєкту).
    pub debug: Option<DebugReport>,
}

#[derive(Serialize, Default)]
pub struct UpdateCheck {
    pub current: String,
    /// Знайдено новішу версію.
    pub available: bool,
    /// Інформація про реліз: версія, дата, повний опис змін.
    pub release: Option<ReleaseInfo>,
    /// Увімкнено автооновлення.
    pub auto: bool,
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
    #[serde(default)]
    pub auto_update_launcher: Option<bool>,
    /// Порожній рядок — видалити збережений ключ.
    #[serde(default)]
    pub curseforge_api_key: Option<String>,
}

async fn build_view(
    paths: &Paths,
    binding: &ProjectBinding,
    entry: &InstanceEntry,
) -> InstanceView {
    let state = instance::load_state(paths, &entry.id).await;
    let remote =
        match manifest::load_cached(&project::remote_manifest_cache(paths, binding, &entry.id)).await {
            Some(remote) => Some(remote),
            // Офлайн без кешу проєкту — беремо копію маніфесту встановленої збірки.
            None => manifest::load_cached(&instance::manifest_cache(paths, &entry.id)).await,
        };
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
        asset_base: project::instance_asset_base(entry),
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
        custom: false,
        mods_count: None,
    }
}

async fn custom_view(paths: &Paths, id: &str, spec: &CustomSpec) -> InstanceView {
    let state = instance::load_state(paths, id).await;
    let installed = state.installed && paths.instance_dir(id).is_dir();
    InstanceView {
        id: id.to_string(),
        name: spec.name.clone(),
        description: String::new(),
        server: String::new(),
        image: String::new(),
        asset_base: String::new(),
        minecraft_version: spec.minecraft_version.clone(),
        loader: spec.loader.clone(),
        loader_version: spec.loader_version.clone(),
        recommended_ram_mb: None,
        size_bytes: None,
        installed,
        local_version: String::new(),
        remote_version: String::new(),
        update_available: false,
        custom: true,
        mods_count: Some(mods::count_enabled(paths, id).await),
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
        settings: settings_view(&config, &paths, max),
        profiles: config.profiles.clone(),
        active_profile: config.active_profile.clone(),
        project: config.project.clone(),
        active_instance: config.active_instance.clone(),
        game_running: state.running_game(),
    })
}

/// Перевіряє нову версію лаунчера через UPDATE_MIRRORS. Якщо увімкнено автооновлення —
/// одразу встановлює її (як раніше), інакше лише повертає інформацію для сповіщення.
#[tauri::command]
pub async fn check_launcher_update(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<UpdateCheck> {
    let (config, _) = load_ctx(&app).await?;
    let mut check = UpdateCheck {
        current: updater::current_version().to_string(),
        auto: config.auto_update_launcher,
        ..UpdateCheck::default()
    };
    status(
        &app,
        tr(
            "Перевірка оновлень лаунчера…",
            "Checking for launcher updates…",
        ),
    );
    let info = match updater::fetch_latest(&state.client).await {
        Ok(info) => info,
        Err(error) => {
            check.error = Some(error.0);
            return Ok(check);
        }
    };
    check.available = updater::is_newer(&info);
    if let Ok(mut slot) = state.update.lock() {
        *slot = check.available.then(|| info.clone());
    }
    check.release = Some(info.clone());
    if !check.available || !config.auto_update_launcher || state.running_game().is_some() {
        return Ok(check);
    }
    // Автооновлення: якщо лаунчер зайнятий іншою дією — лише покажемо сповіщення.
    let Ok(_guard) = state.busy.try_lock() else {
        return Ok(check);
    };
    match updater::install(&app, &state.client, &info).await {
        Ok(UpdateOutcome::Restarting) => check.restarting = true,
        Ok(UpdateOutcome::UpToDate) => check.available = false,
        Err(error) => check.error = Some(error.0),
    }
    Ok(check)
}

/// Кнопка «Завантажити» у сповіщенні або у вікні Changelog.
#[tauri::command]
pub async fn install_launcher_update(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<UpdateCheck> {
    let _guard = state.busy.try_lock().map_err(|_| busy_error())?;
    if state.running_game().is_some() {
        return Err(running_error());
    }
    let (config, _) = load_ctx(&app).await?;
    let cached = state.update.lock().ok().and_then(|slot| slot.clone());
    let info = match cached {
        Some(info) => info,
        None => updater::fetch_latest(&state.client).await?,
    };
    let mut check = UpdateCheck {
        current: updater::current_version().to_string(),
        available: updater::is_newer(&info),
        auto: config.auto_update_launcher,
        release: Some(info.clone()),
        ..UpdateCheck::default()
    };
    match updater::install(&app, &state.client, &info).await? {
        UpdateOutcome::Restarting => check.restarting = true,
        UpdateOutcome::UpToDate => check.available = false,
    }
    Ok(check)
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
pub async fn load_changelog(app: AppHandle, state: State<'_, AppState>) -> Result<Changelog> {
    let (config, paths) = load_ctx(&app).await?;
    Ok(changelog::load(&state.client, &paths, config.project.as_ref()).await)
}

#[tauri::command]
pub fn open_github(repo: String, kind: String, tag: String) -> Result<()> {
    system::open_github(&repo, &kind, &tag)
}

#[tauri::command]
pub async fn load_project(app: AppHandle, state: State<'_, AppState>) -> Result<ProjectView> {
    let (config, paths) = load_ctx(&app).await?;
    let binding = require_project(&config)?;
    let customs = custom::list(&paths).await;
    // Мережа → дзеркала → кеш індексу → лише встановлені збірки. Запуск уже
    // встановлених збірок не блокується, навіть якщо недоступно геть усе.
    let (index, offline, local_only) =
        match project::load_index(&state.client, &paths, &binding).await {
            Ok((index, offline)) => (index, offline, false),
            Err(error) => {
                let local = project::local_index(&paths, &binding).await;
                if local.instances.is_empty() && customs.is_empty() {
                    return Err(error);
                }
                (local, true, true)
            }
        };

    let mut active = config.active_instance.clone();
    if project::find(&index, &active).is_none() && !customs.iter().any(|(id, _)| *id == active) {
        active = index
            .instances
            .first()
            .map(|e| e.id.clone())
            .or_else(|| customs.first().map(|(id, _)| id.clone()))
            .unwrap_or_default();
        // Офлайн (лише встановлені збірки) — вибір гравця в конфігу не перезаписуємо.
        if !local_only {
            let chosen = active.clone();
            update_config(&app, &state, move |config| {
                config.active_instance = chosen;
                Ok(())
            })
            .await?;
        }
    }

    let mut instances = Vec::with_capacity(index.instances.len());
    for entry in &index.instances {
        instances.push(build_view(&paths, &binding, entry).await);
    }
    for (id, spec) in &customs {
        instances.push(custom_view(&paths, id, spec).await);
    }
    let name = project::display_name(&binding, &index);
    if name != binding.name && !local_only {
        // Виправляємо збережену назву («Myteam» → «MyTeam»), щоб вона була правильна й до завантаження.
        let fixed = name.clone();
        let _ = update_config(&app, &state, move |config| {
            if let Some(project) = config.project.as_mut() {
                project.name = fixed;
            }
            Ok(())
        })
        .await;
    }
    Ok(ProjectView {
        code: binding.code.clone(),
        name,
        description: index.description.clone(),
        icon: project::project_icon_url(&binding.index_url),
        offline,
        local_only,
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
    if let Some(spec) = custom::load(&paths, &instance_id).await {
        return Ok(custom_view(&paths, &instance_id, &spec).await);
    }
    let binding = require_project(&config)?;
    let index = match project::load_cache(&paths, &binding).await {
        Some(index) => Some(index),
        None => project::load_index(&state.client, &paths, &binding)
            .await
            .ok()
            .map(|(index, _)| index),
    };
    let entry = match index.as_ref().and_then(|i| project::find(i, &instance_id)).cloned() {
        Some(entry) => entry,
        None => project::find(&project::local_index(&paths, &binding).await, &instance_id)
            .cloned()
            .ctx(tr(
                "Збірку не знайдено в проєкті",
                "Modpack not found in the project",
            ))?,
    };
    if !entry.manifest.is_empty() {
        if let Ok(fresh) = manifest::fetch(&state.client, &entry.manifest).await {
            let _ = manifest::save_cache(
                &project::remote_manifest_cache(&paths, &binding, &entry.id),
                &fresh,
            )
            .await;
        }
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
) -> Result<SettingsView> {
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
    if let Some(key) = &patch.curseforge_api_key {
        validate_api_key(key.trim())?;
    }

    let game_lang_changed = update_config(&app, &state, |config| {
        if let Some(auto) = patch.auto_update_launcher {
            config.auto_update_launcher = auto;
        }
        if let Some(key) = &patch.curseforge_api_key {
            config.curseforge_api_key = key.trim().to_string();
        }
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
    let (config, paths) = load_ctx(&app).await?;
    if let Some(lang) = game_lang_changed {
        for disk in instance::list_on_disk(&paths).await {
            if disk.installed {
                let _ = instance::apply_lang(&paths, &disk.id, &lang).await;
            }
        }
    }
    Ok(settings_view(&config, &paths, max_ram_mb(total_ram_mb())))
}

/// Активні дзеркала, джерело конфігу і хости, що зараз не відповідають.
#[tauri::command]
pub async fn mirrors_info(app: AppHandle) -> Result<MirrorsInfo> {
    let root = Paths::app_root(&app)?;
    Ok(mirrors::info(&root))
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
pub async fn pick_folder(window: WebviewWindow) -> Result<Option<String>> {
    system::pick_folder(
        window_handle(&window),
        tr(
            "Обери папку для лаунчера та збірок",
            "Choose a folder for the launcher and modpacks",
        ),
    )
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

/// Що вдалося дізнатися про збірку проєкту: запис індексу та свіжий manifest.json
/// (через дзеркала). `None` у полях — джерела недоступні, працюємо з локальним кешем.
struct Resolved {
    entry: Option<InstanceEntry>,
    fresh: Option<Manifest>,
}

async fn fetch_manifest_for(
    app: &AppHandle,
    state: &AppState,
    paths: &Paths,
    binding: &ProjectBinding,
    instance_id: &str,
) -> Result<Resolved> {
    step(app, "manifest");
    status(
        app,
        tr("Отримання списку збірок проєкту…", "Loading the project's modpacks…"),
    );
    let (index, index_offline) = match project::load_index(&state.client, paths, binding).await {
        Ok((index, offline)) => (Some(index), offline),
        Err(_) => (None, true),
    };
    if index_offline {
        status(
            app,
            tr(
                "Джерела проєкту недоступні (усі дзеркала) — використовую локальний кеш",
                "Project sources are unreachable (all mirrors) — using the local cache",
            ),
        );
    }
    let Some(entry) = index
        .as_ref()
        .and_then(|index| project::find(index, instance_id))
        .cloned()
    else {
        if index_offline {
            return Ok(Resolved {
                entry: None,
                fresh: None,
            });
        }
        return fail(tr(
            "Збірку не знайдено в проєкті",
            "Modpack not found in the project",
        ));
    };
    status(
        app,
        tr(
            "Перевірка оновлень збірки…",
            "Checking for modpack updates…",
        ),
    );
    let fetched = if entry.manifest.is_empty() {
        None
    } else {
        manifest::fetch(&state.client, &entry.manifest).await.ok()
    };
    match fetched {
        Some(fresh) => {
            status(
                app,
                &t!(
                    "Маніфест отримано: версія {}, файлів {}",
                    "Manifest received: version {}, {} files",
                    if fresh.version.is_empty() { "—" } else { fresh.version.as_str() },
                    fresh.files.len()
                ),
            );
            let _ = manifest::save_cache(
                &project::remote_manifest_cache(paths, binding, &entry.id),
                &fresh,
            )
            .await;
            Ok(Resolved {
                entry: Some(entry),
                fresh: Some(fresh),
            })
        }
        None => {
            status(
                app,
                tr(
                    "manifest.json збірки недоступний на всіх дзеркалах",
                    "The modpack's manifest.json is unreachable on all mirrors",
                ),
            );
            Ok(Resolved {
                entry: Some(entry),
                fresh: None,
            })
        }
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
    paths.ensure_shared_dirs().await?;
    phase(&app, "checking");

    if let Some(spec) = custom::load(&paths, &instance_id).await {
        let mut st = instance::load_state(&paths, &instance_id).await;
        let first_install = !st.installed;
        st.name = spec.name.clone();
        st.project.clear();
        instance::ensure_dirs(&paths, &instance_id).await?;
        instance::save_state(&paths, &instance_id, &st).await?;
        let manifest = spec.to_manifest();
        manifest::save_cache(&instance::manifest_cache(&paths, &instance_id), &manifest).await?;
        start_game(&app, &state, &paths, &config, &profile, &instance_id, &manifest).await?;
        let mut st = instance::load_state(&paths, &instance_id).await;
        if !st.installed {
            st.installed = true;
            instance::save_state(&paths, &instance_id, &st).await?;
        }
        return Ok(PlayOutcome {
            offline: false,
            first_install,
            debug: None,
        });
    }

    let binding = require_project(&config)?;
    let mut st = instance::load_state(&paths, &instance_id).await;
    let first_install = !st.installed;
    let resolved = fetch_manifest_for(&app, &state, &paths, &binding, &instance_id).await?;

    if let Some(entry) = &resolved.entry {
        st.name = entry.name.clone();
    }
    st.project = binding.code.clone();
    instance::ensure_dirs(&paths, &instance_id).await?;
    instance::save_state(&paths, &instance_id, &st).await?;

    let installed_copy = instance::manifest_cache(&paths, &instance_id);
    let (manifest, offline) = match resolved.fresh {
        Some(fresh) => {
            sync::sync_instance(&app, &state.client, &paths, &instance_id, &fresh, false, false)
                .await?;
            manifest::save_cache(&installed_copy, &fresh).await?;
            (fresh, false)
        }
        None => {
            // Усі дзеркала недоступні, але збірку вже встановлено — граємо з локального кешу.
            let cached = if st.installed {
                manifest::load_cached(&installed_copy).await
            } else {
                None
            };
            match cached {
                Some(cached) => {
                    step(&app, "files");
                    status(
                        &app,
                        tr(
                            "Офлайн-режим: джерела недоступні — запуск встановленої збірки з локального кешу",
                            "Offline mode: sources are unreachable — starting the installed modpack from the local cache",
                        ),
                    );
                    (cached, true)
                }
                None => {
                    return fail(tr(
                        "Для встановлення збірки потрібен інтернет: усі джерела й дзеркала недоступні. Перевір підключення або імпортуй збірку з .zip архіву.",
                        "Installing a modpack requires internet: all sources and mirrors are unreachable. Check your connection or import the modpack from a .zip archive.",
                    ))
                }
            }
        }
    };

    // Pre-launch дебагер: звіряє mods/ з manifest.json проєкту (лише для збірок проєкту).
    let debug =
        pack_debugger::run(&app, &state.client, &paths, &instance_id, &manifest, offline).await?;

    start_game(&app, &state, &paths, &config, &profile, &instance_id, &manifest).await?;
    Ok(PlayOutcome {
        offline,
        first_install,
        debug: Some(debug),
    })
}

/// Спільний хвіст запуску: пресет/мова, скін, підготовка версії й старт JVM.
async fn start_game(
    app: &AppHandle,
    state: &AppState,
    paths: &Paths,
    config: &Config,
    profile: &Profile,
    instance_id: &str,
    manifest: &Manifest,
) -> Result<()> {
    let game_dir = paths.instance_dir(instance_id);
    // Пресет графіки й мову гри пишемо лише один раз — коли options.txt ще не існує.
    // Далі гравець сам керує налаштуваннями в грі, лаунчер їх не перезаписує.
    if !game_dir.join("options.txt").exists() {
        instance::apply_preset(paths, instance_id, &config.graphics_preset).await?;
        instance::apply_lang(paths, instance_id, &config.game_lang).await?;
        let mut fresh_state = instance::load_state(paths, instance_id).await;
        fresh_state.preset_applied = true;
        instance::save_state(paths, instance_id, &fresh_state).await?;
    }

    launch::place_skin(
        paths,
        &game_dir,
        &profile.id,
        &profile.nickname,
        profile.has_skin,
    )
    .await;

    let ram = clamp_ram(config.ram_mb);
    let prepared = launch::prepare(app, &state.client, paths, manifest, &game_dir, false).await?;
    launch::spawn(
        app,
        paths,
        state.game.clone(),
        state.processes.clone(),
        &prepared,
        &LaunchRequest {
            instance_id,
            nickname: &profile.nickname,
            ram_mb: ram,
        },
    )
}

/// «Закрити гру» (кнопка-бар'єр поруч із «Гра запущена»): примусово завершує дерево процесів
/// запущеної збірки й чекає, доки лаунчер звільнить стан гри. Інтерфейс, як і при звичайному
/// виході з гри, отримує подію `game-state` зі станом `exited` (без ознаки вильоту).
#[tauri::command]
pub async fn stop_instance(state: State<'_, AppState>, instance_id: String) -> Result<()> {
    check_id(&instance_id)?;
    let is_running = |state: &AppState| state.running_game().as_deref() == Some(instance_id.as_str());
    if !is_running(&state) {
        return Ok(());
    }
    // Запис прибираємо до taskkill: так фонове очікування процесу в launch::spawn знає,
    // що гру закрив гравець, і не показує вікно вильоту.
    let pid = state
        .processes
        .lock()
        .ok()
        .and_then(|mut map| map.remove(&instance_id));
    let killed = match pid {
        Some(pid) => launch::kill_tree(pid).await,
        None => Ok(()),
    };
    for _ in 0..50 {
        if !is_running(&state) {
            return Ok(());
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    // Гра досі працює — повертаємо PID, щоб можна було спробувати ще раз.
    if let Some(pid) = pid {
        if let Ok(mut map) = state.processes.lock() {
            map.entry(instance_id.clone()).or_insert(pid);
        }
    }
    killed?;
    fail(tr(
        "Гра не відповідає — спробуй закрити її ще раз",
        "The game is not responding — try closing it again",
    ))
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
    phase(&app, "checking");
    if let Some(spec) = custom::load(&paths, &instance_id).await {
        // У власній збірці лаунчер не керує модами — перевіряємо лише гру й завантажувач.
        let manifest = spec.to_manifest();
        let game_dir = paths.instance_dir(&instance_id);
        launch::prepare(&app, &state.client, &paths, &manifest, &game_dir, true).await?;
        status(&app, tr("Перевірку завершено", "Check complete"));
        return Ok(SyncReport::default());
    }
    let binding = require_project(&config)?;
    let resolved = fetch_manifest_for(&app, &state, &paths, &binding, &instance_id).await?;
    let manifest = resolved.fresh.ctx(tr(
        "Для перевірки файлів потрібен інтернет: усі джерела й дзеркала недоступні",
        "Checking files requires internet: all sources and mirrors are unreachable",
    ))?;
    let report =
        sync::sync_instance(&app, &state.client, &paths, &instance_id, &manifest, true, true)
            .await?;
    manifest::save_cache(&instance::manifest_cache(&paths, &instance_id), &manifest).await?;
    let game_dir = paths.instance_dir(&instance_id);
    launch::prepare(&app, &state.client, &paths, &manifest, &game_dir, true).await?;
    status(&app, tr("Перевірку завершено", "Check complete"));
    Ok(report)
}

/// «Перевірити збірку (Дебагер)»: повний звіт про mods/ встановленої збірки проєкту
/// з автоматичним виправленням (докачування — через дзеркала).
#[tauri::command]
pub async fn debug_instance(
    app: AppHandle,
    state: State<'_, AppState>,
    instance_id: String,
) -> Result<DebugReport> {
    check_id(&instance_id)?;
    let _guard = state.busy.try_lock().map_err(|_| busy_error())?;
    if state.running_game().as_deref() == Some(instance_id.as_str()) {
        return Err(running_error());
    }
    let (_, paths) = load_ctx(&app).await?;
    if custom::load(&paths, &instance_id).await.is_some() {
        return fail(tr(
            "Дебагер перевіряє лише збірки проєкту — власні збірки він не чіпає",
            "The debugger only checks project modpacks — it never touches custom ones",
        ));
    }
    let st = instance::load_state(&paths, &instance_id).await;
    let manifest = if st.installed {
        manifest::load_cached(&instance::manifest_cache(&paths, &instance_id)).await
    } else {
        None
    };
    let manifest = manifest.ctx(tr(
        "Збірку ще не встановлено — дебагеру нема з чим звіряти",
        "The modpack isn't installed yet — there's nothing to check against",
    ))?;
    phase(&app, "checking");
    pack_debugger::run(&app, &state.client, &paths, &instance_id, &manifest, false).await
}

#[tauri::command]
pub async fn pick_zip(window: WebviewWindow) -> Result<Option<String>> {
    system::pick_zip(
        window_handle(&window),
        tr("Обери .zip архів збірки", "Choose the modpack .zip archive"),
        tr("Архів збірки", "Modpack archive"),
    )
    .await
}

/// Імпорт збірки з .zip: у вибрану збірку (`instance_id`) або як нову власну збірку
/// (`instance_id` порожній). `base_path` — папка кореня збірки всередині архіву
/// (порожньо — знайти автоматично за manifest.json / mods/).
#[tauri::command]
pub async fn import_local_pack(
    app: AppHandle,
    state: State<'_, AppState>,
    zip_path: String,
    instance_id: Option<String>,
    base_path: Option<String>,
) -> Result<ImportReport> {
    let _guard = state.busy.try_lock().map_err(|_| busy_error())?;
    let (config, paths) = load_ctx(&app).await?;
    paths.ensure_shared_dirs().await?;

    let zip = PathBuf::from(zip_path.trim());
    let is_zip = zip
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.eq_ignore_ascii_case("zip"))
        .unwrap_or(false);
    if !zip.is_absolute() || !zip.is_file() || !is_zip {
        return fail(tr("Обери .zip архів збірки", "Choose the modpack .zip archive"));
    }

    status(&app, tr("Перевірка архіву…", "Checking the archive…"));
    let (probe, base) = (zip.clone(), base_path.unwrap_or_default());
    let inspected = tokio::task::spawn_blocking(move || import_pack::inspect(&probe, &base)).await??;

    let target = instance_id
        .map(|id| id.trim().to_string())
        .filter(|id| !id.is_empty());
    let mut report = ImportReport::default();
    let (id, project_pack) = match target {
        Some(id) => {
            check_id(&id)?;
            if state.running_game().as_deref() == Some(id.as_str()) {
                return Err(running_error());
            }
            let is_custom = custom::load(&paths, &id).await.is_some();
            if !is_custom && inspected.manifest.is_none() {
                return fail(tr(
                    "В архіві немає manifest.json — без нього збірку проєкту не встановити",
                    "The archive has no manifest.json — a project modpack can't be installed without it",
                ));
            }
            if !is_custom {
                require_project(&config)?;
            }
            (id, !is_custom)
        }
        None => {
            let manifest = inspected.manifest.as_ref().ctx(tr(
                "Щоб створити нову збірку з архіву, у ньому має бути manifest.json",
                "To create a new modpack from an archive, it must contain manifest.json",
            ))?;
            let kind = LoaderKind::parse(&manifest.loader.kind)?;
            let fallback = zip
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| "Imported".to_string());
            let spec = CustomSpec {
                name: if manifest.name.trim().is_empty() {
                    fallback
                } else {
                    manifest.name.trim().to_string()
                },
                minecraft_version: manifest.minecraft_version.clone(),
                loader: kind.id().to_string(),
                loader_version: manifest.loader.version.clone(),
                created: 0,
            };
            report.created = true;
            (custom::create(&paths, spec).await?, false)
        }
    };
    instance::ensure_dirs(&paths, &id).await?;

    status(
        &app,
        tr(
            "Розпаковування mods/, config/, resourcepacks/, shaderpacks/…",
            "Extracting mods/, config/, resourcepacks/, shaderpacks/…",
        ),
    );
    let created = report.created;
    let (source, prefix, dir) = (zip.clone(), inspected.prefix.clone(), paths.instance_dir(&id));
    let extracted = tokio::task::spawn_blocking(move || {
        let mut report = report;
        let written = import_pack::extract(&source, &prefix, &dir, &mut report)?;
        Ok::<_, AppError>((written, report))
    })
    .await?;
    let (written, mut report) = match extracted {
        Ok(done) => done,
        Err(error) => {
            // Нова збірка з битого архіву не повинна лишатися порожньою в списку.
            if created {
                let _ = instance::delete(&paths, &id).await;
            }
            return Err(error);
        }
    };
    report.instance_id = id.clone();

    let mut st = instance::load_state(&paths, &id).await;
    if let Some(manifest) = &inspected.manifest {
        let expected: std::collections::HashMap<&str, &str> = manifest
            .files
            .iter()
            .map(|f| (f.path.as_str(), f.sha256.as_str()))
            .collect();
        for (path, sha) in &written {
            match expected.get(path.as_str()) {
                Some(want) if want.eq_ignore_ascii_case(sha) => {
                    report.verified += 1;
                    st.files.insert(path.clone(), sha.clone());
                }
                Some(_) => report.mismatched.push(path.clone()),
                None => {}
            }
        }
        manifest::save_cache(&instance::manifest_cache(&paths, &id), manifest).await?;
        report.pack_version = manifest.version.clone();
        st.pack_version = manifest.version.clone();
    }
    st.installed = true;
    st.kept_personal = false;
    if project_pack {
        let binding = require_project(&config)?;
        let cached_name = project::load_cache(&paths, &binding)
            .await
            .and_then(|index| project::find(&index, &id).map(|entry| entry.name.clone()));
        st.project = binding.code.clone();
        st.name = cached_name
            .or_else(|| inspected.manifest.as_ref().map(|m| m.name.trim().to_string()))
            .filter(|name| !name.is_empty())
            .unwrap_or_else(|| id.clone());
    } else {
        st.project.clear();
        if let Some(spec) = custom::load(&paths, &id).await {
            st.name = spec.name;
        }
    }
    instance::save_state(&paths, &id, &st).await?;
    report.name = st.name.clone();

    if report.created {
        let chosen = id.clone();
        update_config(&app, &state, move |config| {
            config.active_instance = chosen;
            Ok(())
        })
        .await?;
    }
    status(
        &app,
        &t!(
            "Збірку «{}» імпортовано з архіву: файлів — {}",
            "Modpack \"{}\" imported from the archive: {} files",
            report.name,
            report.files
        ),
    );
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
    keep_personal: Option<bool>,
) -> Result<()> {
    check_id(&instance_id)?;
    let _guard = state.busy.try_lock().map_err(|_| busy_error())?;
    if state.running_game().as_deref() == Some(instance_id.as_str()) {
        return Err(running_error());
    }
    let (_, paths) = load_ctx(&app).await?;
    if keep_personal.unwrap_or(false) {
        // Опис власної збірки зберігаємо, щоб світи не «осиротіли» і збірку можна було відновити.
        let spec = custom::load(&paths, &instance_id).await;
        instance::delete_keeping_personal(&paths, &instance_id).await?;
        if let Some(spec) = spec {
            custom::save(&paths, &instance_id, &spec).await?;
        }
        Ok(())
    } else {
        instance::delete(&paths, &instance_id).await
    }
}

// ───────────────────────────── власні збірки ─────────────────────────────

#[tauri::command]
pub async fn minecraft_versions(
    state: State<'_, AppState>,
    snapshots: Option<bool>,
) -> Result<Vec<McVersion>> {
    custom::minecraft_versions(&state.client, snapshots.unwrap_or(false)).await
}

#[tauri::command]
pub async fn loader_versions(
    state: State<'_, AppState>,
    loader: String,
    minecraft_version: String,
) -> Result<Vec<LoaderVersion>> {
    custom::loader_versions(&state.client, &loader, &minecraft_version).await
}

async fn require_custom(app: &AppHandle, instance_id: &str) -> Result<(Config, Paths, CustomSpec)> {
    check_id(instance_id)?;
    let (config, paths) = load_ctx(app).await?;
    let spec = custom::load(&paths, instance_id).await.ctx(tr(
        "Моди можна додавати лише у власні збірки: склад збірки проєкту визначає її manifest.json",
        "Mods can only be added to custom modpacks: a project modpack is defined by its manifest.json",
    ))?;
    Ok((config, paths, spec))
}

fn ensure_not_running(state: &AppState, instance_id: &str) -> Result<()> {
    if state.running_game().as_deref() == Some(instance_id) {
        return Err(running_error());
    }
    Ok(())
}

#[derive(Serialize)]
pub struct ModSources {
    pub modrinth: bool,
    /// Є ключ CurseForge (у налаштуваннях або змінній середовища).
    /// Завжди `false`, поки CurseForge вимкнено прапорцем `ENABLE_CURSEFORGE`.
    pub curseforge: bool,
}

#[tauri::command]
pub async fn mod_sources(app: AppHandle) -> Result<ModSources> {
    let (config, _) = load_ctx(&app).await?;
    Ok(ModSources {
        modrinth: true,
        curseforge: ENABLE_CURSEFORGE && config.curseforge_key().is_some(),
    })
}

fn mod_target(spec: &CustomSpec) -> Result<Target> {
    Target::new(&spec.minecraft_version, &spec.loader)
}

#[tauri::command]
pub async fn mod_categories(
    app: AppHandle,
    state: State<'_, AppState>,
    source: Option<String>,
) -> Result<Vec<mods::Category>> {
    let (config, _) = load_ctx(&app).await?;
    let key = config.curseforge_key();
    let api = Api {
        client: &state.client,
        source: Source::parse(source.as_deref().unwrap_or(""))?,
        key: key.as_deref(),
    };
    mods::categories(&api).await
}

#[tauri::command]
pub async fn search_mods(
    app: AppHandle,
    state: State<'_, AppState>,
    instance_id: String,
    source: Option<String>,
    query: String,
    category: Option<String>,
    offset: Option<u32>,
) -> Result<mods::SearchPage> {
    let (config, _, spec) = require_custom(&app, &instance_id).await?;
    let target = mod_target(&spec)?;
    let key = config.curseforge_key();
    let api = Api {
        client: &state.client,
        source: Source::parse(source.as_deref().unwrap_or(""))?,
        key: key.as_deref(),
    };
    let query: String = query.chars().take(100).collect();
    mods::search(&api, &target, &query, category.as_deref(), offset.unwrap_or(0)).await
}

/// Версії файлу мода, сумісні з версією Minecraft і завантажувачем збірки.
#[tauri::command]
pub async fn mod_versions(
    app: AppHandle,
    state: State<'_, AppState>,
    instance_id: String,
    source: Option<String>,
    project_id: String,
) -> Result<Vec<mods::ModVersion>> {
    mods::check_api_id(&project_id)?;
    let (config, _, spec) = require_custom(&app, &instance_id).await?;
    let target = mod_target(&spec)?;
    let key = config.curseforge_key();
    let api = Api {
        client: &state.client,
        source: Source::parse(source.as_deref().unwrap_or(""))?,
        key: key.as_deref(),
    };
    mods::versions(&api, &target, &project_id).await
}

/// Власна збірка — моди з записами лаунчера; збірка проєкту — склад з manifest.json (лише перегляд).
#[tauri::command]
pub async fn list_mods(app: AppHandle, instance_id: String) -> Result<Vec<mods::ModEntry>> {
    check_id(&instance_id)?;
    let (_, paths) = load_ctx(&app).await?;
    if custom::load(&paths, &instance_id).await.is_some() {
        return Ok(mods::list(&paths, &instance_id).await);
    }
    let manifest = manifest::load_cached(&instance::manifest_cache(&paths, &instance_id)).await;
    Ok(mods::list_managed(&paths, &instance_id, manifest.as_ref()).await)
}

#[tauri::command]
pub async fn install_mod(
    app: AppHandle,
    state: State<'_, AppState>,
    instance_id: String,
    source: Option<String>,
    project_id: String,
    version_id: Option<String>,
) -> Result<mods::InstallReport> {
    let (config, paths, spec) = require_custom(&app, &instance_id).await?;
    ensure_not_running(&state, &instance_id)?;
    mods::check_api_id(&project_id)?;
    let version_id = version_id.filter(|v| !v.is_empty());
    if let Some(version) = &version_id {
        mods::check_api_id(version)?;
    }
    let target = mod_target(&spec)?;
    let key = config.curseforge_key();
    let api = Api {
        client: &state.client,
        source: Source::parse(source.as_deref().unwrap_or(""))?,
        key: key.as_deref(),
    };
    let _lock = state.mods_lock.lock().await;
    mods::install(&api, &paths, &instance_id, &target, &project_id, version_id).await
}

#[tauri::command]
pub async fn set_mod_enabled(
    app: AppHandle,
    state: State<'_, AppState>,
    instance_id: String,
    filename: String,
    enabled: bool,
) -> Result<()> {
    let (_, paths, _) = require_custom(&app, &instance_id).await?;
    ensure_not_running(&state, &instance_id)?;
    let _lock = state.mods_lock.lock().await;
    mods::set_enabled(&paths, &instance_id, &filename, enabled).await
}

#[tauri::command]
pub async fn remove_mod(
    app: AppHandle,
    state: State<'_, AppState>,
    instance_id: String,
    filename: String,
) -> Result<()> {
    let (_, paths, _) = require_custom(&app, &instance_id).await?;
    ensure_not_running(&state, &instance_id)?;
    let _lock = state.mods_lock.lock().await;
    mods::remove(&paths, &instance_id, &filename).await
}

#[tauri::command]
pub async fn open_mods_folder(app: AppHandle, instance_id: String) -> Result<()> {
    check_id(&instance_id)?;
    let (_, paths) = load_ctx(&app).await?;
    let dir = paths.instance_dir(&instance_id).join("mods");
    tokio::fs::create_dir_all(&dir).await?;
    system::open_folder(&dir)
}

#[tauri::command]
pub async fn create_custom_instance(
    app: AppHandle,
    state: State<'_, AppState>,
    spec: CustomSpec,
) -> Result<InstanceView> {
    let (_, paths) = load_ctx(&app).await?;
    paths.ensure_shared_dirs().await?;
    let id = custom::create(&paths, spec).await?;
    let chosen = id.clone();
    update_config(&app, &state, move |config| {
        config.active_instance = chosen;
        Ok(())
    })
    .await?;
    let spec = custom::load(&paths, &id).await.ctx("custom.json")?;
    Ok(custom_view(&paths, &id, &spec).await)
}

/// Скільки місця звільнить видалення і скільки займають світи/скріншоти.
#[tauri::command]
pub async fn instance_disk_usage(app: AppHandle, instance_id: String) -> Result<DiskUsage> {
    check_id(&instance_id)?;
    let (_, paths) = load_ctx(&app).await?;
    Ok(instance::disk_usage(&paths, &instance_id).await)
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
