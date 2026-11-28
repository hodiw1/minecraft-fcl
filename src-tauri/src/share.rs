//! Спільні збірки: власна збірка → легкий пакет для друзів → Supabase Storage.
//!
//! Пакет — це `.zip` до 45 МБ:
//! ```text
//! terrapolis-pack.json   версія гри, завантажувач, список модів/ресурс-паків/шейдерів
//!                        з CDN-посиланнями (Modrinth / CurseForge) і контрольними сумами
//! overrides/             config/, defaultconfigs/, kubejs/, servers.dat… та файли контенту,
//!                        яких немає на CDN (додані вручну)
//! ```
//! Друг завантажує архів за шляхом, який видає `get_pack_download` (лише після перевірки
//! дружби й доступу), а встановлення йде тим самим безпечним шляхом, що й готові збірки
//! з Modrinth (`modpack::install_shared`): кожен шлях і хост перевіряються, хеш обов'язковий.

use std::collections::{HashMap, HashSet};
use std::io::Write;
use std::path::{Path, PathBuf};

use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tauri::{AppHandle, Manager, State};

use crate::commands::{busy_error, custom_view, load_ctx, update_config, InstanceView, ModpackInstalled};
use crate::config::Config;
use crate::constants::MAX_SHARED_PACK_BYTES;
use crate::custom::{self, is_uuid, CustomSpec, VISIBILITIES};
use crate::error::{fail, AppError, Context, Result};
use crate::events::status;
use crate::i18n::tr;
use crate::modpack;
use crate::mods::{self, Api, ContentKind, ModRecord, Resolution, Source, Target};
use crate::paths::Paths;
use crate::project;
use crate::social::{self, rpc_as, CloudError};
use crate::state::AppState;
use crate::t;
use crate::util::{download_file, noop, sha256_file, Checksum};

pub const SHARE_MANIFEST: &str = "terrapolis-pack.json";
pub const SHARE_FORMAT: u32 = 1;
/// Запас під службові записи zip: сам вміст не більший за це значення.
const CONTENT_BUDGET: u64 = MAX_SHARED_PACK_BYTES - 512 * 1024;
const MAX_OVERRIDE_FILES: usize = 4_000;
const MAX_DEPTH: usize = 12;

/// Теки, які варто передати другові разом зі збіркою.
const OVERRIDE_DIRS: [&str; 7] = [
    "config",
    "defaultconfigs",
    "kubejs",
    "scripts",
    "global_packs",
    "openloader",
    "patchouli_books",
];
/// Окремі файли в корені збірки. options.txt не передаємо: у друга свої клавіші,
/// графіка та мова (лаунчер сам застосує його пресет під час першого запуску).
const OVERRIDE_FILES: [&str; 1] = ["servers.dat"];
const CONTENT_KINDS: [ContentKind; 3] = [ContentKind::Mod, ContentKind::ResourcePack, ContentKind::Shader];

// ───────────────────────────── формат пакета ─────────────────────────────

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct ShareFile {
    /// `mods/sodium.jar`, `resourcepacks/x.zip`, `shaderpacks/y.zip`.
    pub path: String,
    pub url: String,
    #[serde(default)]
    pub sha1: String,
    #[serde(default)]
    pub sha512: String,
    #[serde(default)]
    pub size: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct ShareManifest {
    pub format: u32,
    pub name: String,
    pub minecraft_version: String,
    #[serde(default)]
    pub loader: String,
    #[serde(default)]
    pub loader_version: String,
    #[serde(default)]
    pub files: Vec<ShareFile>,
    /// Записи менеджера контенту — щоб у друга моди показувались з назвами й іконками.
    #[serde(default)]
    pub mods: Vec<ModRecord>,
    #[serde(default)]
    pub resourcepacks: Vec<ModRecord>,
    #[serde(default)]
    pub shaderpacks: Vec<ModRecord>,
}

impl ShareManifest {
    pub fn records_mut(&mut self, kind: ContentKind) -> &mut Vec<ModRecord> {
        match kind {
            ContentKind::ResourcePack => &mut self.resourcepacks,
            ContentKind::Shader => &mut self.shaderpacks,
            _ => &mut self.mods,
        }
    }
}

/// Файл з папки збірки, кандидат в overrides/.
#[derive(Clone, Debug)]
struct LocalFile {
    relative: String,
    absolute: PathBuf,
    size: u64,
    modified: u64,
}

fn modified_secs(meta: &std::fs::Metadata) -> u64 {
    meta.modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn walk(base: &Path, dir: &Path, depth: usize, out: &mut Vec<LocalFile>) {
    if depth > MAX_DEPTH || out.len() >= MAX_OVERRIDE_FILES {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') || name.ends_with(".part") || name.ends_with(".tmp") {
            continue;
        }
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        if kind.is_symlink() {
            continue;
        }
        let path = entry.path();
        if kind.is_dir() {
            walk(base, &path, depth + 1, out);
        } else if kind.is_file() {
            let Ok(relative) = path.strip_prefix(base) else {
                continue;
            };
            let relative = relative.to_string_lossy().replace('\\', "/");
            if modpack::safe_pack_path(&relative).is_none() {
                continue;
            }
            let Ok(meta) = entry.metadata() else {
                continue;
            };
            out.push(LocalFile {
                relative,
                absolute: path,
                size: meta.len(),
                modified: modified_secs(&meta),
            });
            if out.len() >= MAX_OVERRIDE_FILES {
                return;
            }
        }
    }
}

/// Усе, що може потрапити в пакет: overrides (налаштування) і увімкнений контент.
fn collect(dir: &Path) -> (Vec<LocalFile>, Vec<LocalFile>) {
    let mut overrides = Vec::new();
    for name in OVERRIDE_DIRS {
        walk(dir, &dir.join(name), 1, &mut overrides);
    }
    for name in OVERRIDE_FILES {
        let path = dir.join(name);
        if let Ok(meta) = std::fs::metadata(&path) {
            if meta.is_file() {
                overrides.push(LocalFile {
                    relative: name.to_string(),
                    absolute: path,
                    size: meta.len(),
                    modified: modified_secs(&meta),
                });
            }
        }
    }
    // Іконка збірки (.launcher/icon.png) їде як overrides/icon.png — у друга лаунчер
    // перевірить її й перенесе на своє місце (modpack::install_shared).
    let icon = dir.join(".launcher").join(crate::custom::ICON_FILE);
    if let Ok(meta) = std::fs::metadata(&icon) {
        if meta.is_file() && meta.len() <= crate::custom::MAX_ICON_BYTES as u64 {
            overrides.push(LocalFile {
                relative: crate::custom::ICON_FILE.to_string(),
                absolute: icon,
                size: meta.len(),
                modified: modified_secs(&meta),
            });
        }
    }
    let mut content = Vec::new();
    for kind in CONTENT_KINDS {
        let Ok(entries) = std::fs::read_dir(dir.join(kind.folder())) else {
            continue;
        };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            let Ok(meta) = entry.metadata() else {
                continue;
            };
            // Лише увімкнені файли (вимкнені мають суфікс .disabled).
            if !meta.is_file() || !name.to_ascii_lowercase().ends_with(kind.extension()) {
                continue;
            }
            let relative = format!("{}/{name}", kind.folder());
            if modpack::safe_pack_path(&relative).is_none() {
                continue;
            }
            content.push(LocalFile {
                relative,
                absolute: entry.path(),
                size: meta.len(),
                modified: modified_secs(&meta),
            });
        }
    }
    overrides.sort_by(|a, b| a.relative.cmp(&b.relative));
    content.sort_by(|a, b| a.relative.cmp(&b.relative));
    (overrides, content)
}

/// Відбиток вмісту: якщо змінився з моменту публікації — копію для друзів треба оновити.
fn fingerprint_of(spec: &CustomSpec, overrides: &[LocalFile], content: &[LocalFile]) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(format!(
        "{}\n{}\n{}\n{}\n{}\n",
        spec.name, spec.minecraft_version, spec.loader, spec.loader_version, spec.visibility
    ));
    for file in overrides.iter().chain(content) {
        hasher.update(format!("{}|{}|{}\n", file.relative, file.size, file.modified));
    }
    hex::encode(hasher.finalize())
}

pub async fn fingerprint(paths: &Paths, id: &str, spec: &CustomSpec) -> String {
    let dir = paths.instance_dir(id);
    let spec = spec.clone();
    tokio::task::spawn_blocking(move || {
        let (overrides, content) = collect(&dir);
        fingerprint_of(&spec, &overrides, &content)
    })
    .await
    .unwrap_or_default()
}

// ───────────────────────────── CDN-посилання ─────────────────────────────

#[derive(Deserialize)]
struct MrVersion {
    id: String,
    #[serde(default)]
    files: Vec<MrVersionFile>,
}

#[derive(Deserialize)]
struct MrVersionFile {
    url: String,
    filename: String,
    #[serde(default)]
    size: u64,
    #[serde(default)]
    hashes: HashMap<String, String>,
}

/// Версії Modrinth пачками (`GET /v2/versions?ids=[...]`) — один запит на 100 модів.
async fn modrinth_files(client: &reqwest::Client, ids: &[String]) -> HashMap<String, MrVersion> {
    let mut out = HashMap::new();
    for chunk in ids.chunks(100) {
        let ids = serde_json::to_string(chunk).unwrap_or_default();
        let response = client
            .get("https://api.modrinth.com/v2/versions")
            .query(&[("ids", ids)])
            .timeout(std::time::Duration::from_secs(20))
            .send()
            .await;
        let Ok(response) = response.and_then(|r| r.error_for_status()) else {
            continue;
        };
        if let Ok(list) = response.json::<Vec<MrVersion>>().await {
            for version in list {
                out.insert(version.id.clone(), version);
            }
        }
    }
    out
}

/// Запис для маніфесту — лише https-посилання на CDN Modrinth/CurseForge з sha1/sha512;
/// усе інше (github, дзеркала, файли без хешу) їде в архів, де його перевіряє друг.
fn share_file(relative: &str, url: &str, checksum: &Checksum, size: u64) -> Option<ShareFile> {
    if !modpack::allowed_share_url(url) {
        return None;
    }
    let (sha1, sha512) = match checksum {
        Checksum::Sha1(v) if v.len() == 40 => (v.to_lowercase(), String::new()),
        Checksum::Sha512(v) if v.len() == 128 => (String::new(), v.to_lowercase()),
        _ => return None,
    };
    Some(ShareFile {
        path: relative.to_string(),
        url: url.to_string(),
        sha1,
        sha512,
        size,
    })
}

/// Звіт пакування.
pub struct Packed {
    pub archive: PathBuf,
    pub size: u64,
    pub sha256: String,
    pub fingerprint: String,
    /// Файлів, які друг завантажить із CDN.
    pub cdn: usize,
    /// Файлів, покладених в архів.
    pub bundled: usize,
    /// Не влізло в 45 МБ.
    pub skipped: Vec<String>,
}

/// Пакує власну збірку: маніфест з CDN-посиланнями + overrides/ (до 45 МБ).
pub async fn pack(state: &AppState, config: &Config, paths: &Paths, id: &str, spec: &CustomSpec) -> Result<Packed> {
    let dir = paths.instance_dir(id);
    let (overrides, content) = {
        let dir = dir.clone();
        tokio::task::spawn_blocking(move || collect(&dir)).await?
    };
    let fingerprint = fingerprint_of(spec, &overrides, &content);
    let present: HashSet<String> = content.iter().map(|f| f.relative.to_ascii_lowercase()).collect();

    let mut manifest = ShareManifest {
        format: SHARE_FORMAT,
        name: spec.name.clone(),
        minecraft_version: spec.minecraft_version.clone(),
        loader: spec.loader.clone(),
        loader_version: spec.loader_version.clone(),
        ..ShareManifest::default()
    };

    // Записи менеджера контенту для увімкнених файлів.
    let mut records: Vec<(ContentKind, ModRecord)> = Vec::new();
    for kind in CONTENT_KINDS {
        for record in mods::load_records(paths, id, kind).await {
            let relative = format!("{}/{}", kind.folder(), record.filename);
            if present.contains(&relative.to_ascii_lowercase()) {
                records.push((kind, record));
            }
        }
    }

    // Modrinth — пачкою, CurseForge — по одному (потрібен ключ власника; без нього файл іде в архів).
    let modrinth_ids: Vec<String> = records
        .iter()
        .filter(|(_, r)| r.source_id() == "modrinth" && mods::check_api_id(&r.version_id).is_ok())
        .map(|(_, r)| r.version_id.clone())
        .collect();
    let versions = if modrinth_ids.is_empty() {
        HashMap::new()
    } else {
        modrinth_files(&state.client, &modrinth_ids).await
    };
    let key = config.curseforge_key();
    let curseforge: Vec<(usize, Option<ShareFile>)> = {
        let jobs: Vec<(usize, ContentKind, ModRecord)> = records
            .iter()
            .enumerate()
            .filter(|(_, (_, r))| r.source_id() == "curseforge")
            .map(|(i, (k, r))| (i, *k, r.clone()))
            .collect();
        let api = Api {
            client: &state.client,
            source: Source::CurseForge,
            key: key.as_deref(),
        };
        let api = &api;
        futures_util::stream::iter(jobs.into_iter().map(|(index, kind, record)| async move {
            if api.key.is_none()
                || mods::check_api_id(&record.project_id).is_err()
                || mods::check_api_id(&record.version_id).is_err()
            {
                return (index, None);
            }
            let Ok(target) = Target::content(kind, &spec.minecraft_version, &spec.loader) else {
                return (index, None);
            };
            let relative = format!("{}/{}", kind.folder(), record.filename);
            let file = match mods::resolve(api, &target, &record.project_id, Some(&record.version_id)).await {
                Ok(Resolution::File(file)) if file.filename.eq_ignore_ascii_case(&record.filename) => {
                    share_file(&relative, &file.url, &file.checksum, 0)
                }
                _ => None,
            };
            (index, file)
        }))
        .buffer_unordered(4)
        .collect()
        .await
    };
    let curseforge: HashMap<usize, ShareFile> =
        curseforge.into_iter().filter_map(|(i, f)| f.map(|f| (i, f))).collect();

    let mut on_cdn: HashSet<String> = HashSet::new();
    for (index, (kind, record)) in records.iter().enumerate() {
        let relative = format!("{}/{}", kind.folder(), record.filename);
        let file = if record.source_id() == "modrinth" {
            versions.get(&record.version_id).and_then(|version| {
                let file = version
                    .files
                    .iter()
                    .find(|f| f.filename.eq_ignore_ascii_case(&record.filename))?;
                let checksum = match (file.hashes.get("sha512"), file.hashes.get("sha1")) {
                    (Some(h), _) => Checksum::Sha512(h.clone()),
                    (_, Some(h)) => Checksum::Sha1(h.clone()),
                    _ => Checksum::None,
                };
                share_file(&relative, &file.url, &checksum, file.size)
            })
        } else {
            curseforge.get(&index).cloned()
        };
        let mut record = record.clone();
        record.icon_url = record.icon_url.filter(|u| u.starts_with("https://") && u.len() < 400);
        if let Some(file) = file {
            on_cdn.insert(relative.to_ascii_lowercase());
            manifest.files.push(file);
        }
        manifest.records_mut(*kind).push(record);
    }

    // Архів: спершу налаштування (вони маленькі й важливі), потім файли без CDN.
    let mut budget = CONTENT_BUDGET;
    let mut bundle: Vec<LocalFile> = Vec::new();
    let mut skipped: Vec<String> = Vec::new();
    let leftovers = content
        .into_iter()
        .filter(|f| !on_cdn.contains(&f.relative.to_ascii_lowercase()));
    for file in overrides.into_iter().chain(leftovers) {
        if file.size <= budget {
            budget -= file.size;
            bundle.push(file);
        } else {
            skipped.push(file.relative);
        }
    }

    let shared = paths.cache.join("shared");
    tokio::fs::create_dir_all(&shared).await?;
    let archive = shared.join(format!("{id}.zip"));
    let cdn = manifest.files.len();
    let bundled = bundle.len();
    let text = serde_json::to_vec_pretty(&manifest)?;
    {
        let archive = archive.clone();
        tokio::task::spawn_blocking(move || write_archive(&archive, &text, &bundle)).await??;
    }
    let size = tokio::fs::metadata(&archive).await?.len();
    if size > MAX_SHARED_PACK_BYTES {
        let _ = tokio::fs::remove_file(&archive).await;
        return fail(tr(
            "Архів збірки завеликий (понад 45 МБ)",
            "The modpack archive is too large (over 45 MB)",
        ));
    }
    let sha256 = {
        let archive = archive.clone();
        tokio::task::spawn_blocking(move || sha256_file(&archive)).await??
    };
    Ok(Packed {
        archive,
        size,
        sha256,
        fingerprint,
        cdn,
        bundled,
        skipped,
    })
}

fn write_archive(archive: &Path, manifest: &[u8], files: &[LocalFile]) -> Result<()> {
    use zip::write::SimpleFileOptions;
    use zip::CompressionMethod;
    let temp = archive.with_extension("zip.part");
    let out = std::fs::File::create(&temp)?;
    let mut zip = zip::ZipWriter::new(out);
    let deflate = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    // .jar/.zip уже стиснуті — повторне стиснення лише витрачає час.
    let stored = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
    zip.start_file(SHARE_MANIFEST, deflate)?;
    zip.write_all(manifest)?;
    for file in files {
        let lower = file.relative.to_ascii_lowercase();
        let options = if lower.ends_with(".jar") || lower.ends_with(".zip") { stored } else { deflate };
        let Ok(mut source) = std::fs::File::open(&file.absolute) else {
            continue;
        };
        zip.start_file(format!("overrides/{}", file.relative), options)?;
        std::io::copy(&mut source, &mut zip)?;
    }
    zip.finish()?;
    std::fs::rename(&temp, archive)?;
    Ok(())
}

// ───────────────────────────── публікація ─────────────────────────────

fn cloud(error: CloudError) -> AppError {
    AppError(error.to_string())
}

fn offline_account() -> AppError {
    AppError(
        tr(
            "Акаунт ще не синхронізовано з хмарою — збірку буде опубліковано, щойно з'явиться інтернет",
            "The account isn't synced with the cloud yet — the modpack will be published once you're online",
        )
        .to_string(),
    )
}

#[derive(Serialize, Default)]
pub struct PublishReport {
    pub cdn: usize,
    pub bundled: usize,
    pub skipped: Vec<String>,
    pub size_bytes: u64,
}

/// Публікує (або оновлює) спільну копію збірки: пакування → `publish_pack` → Storage → `finish_pack_upload`.
async fn publish(state: &AppState, config: &Config, paths: &Paths, id: &str) -> Result<PublishReport> {
    let auth = config.online_account().ok_or_else(offline_account)?;
    let _share = state.social.share_lock.lock().await;
    let mut spec = custom::load(paths, id).await.ctx(tr("Збірку не знайдено", "Modpack not found"))?;
    if !spec.is_shared() {
        return Ok(PublishReport::default());
    }
    let packed = {
        // Моди не змінюються, поки збираємо архів.
        let _mods = state.mods_lock.lock().await;
        pack(state, config, paths, id, &spec).await?
    };
    let outcome: Result<PublishReport> = async {
        let ticket = rpc_as(
            &state.client,
            &auth,
            "publish_pack",
            json!({
                "p_local_id": id,
                "p_name": spec.name,
                "p_visibility": spec.visibility,
                "p_minecraft_version": spec.minecraft_version,
                "p_loader": spec.loader,
                "p_loader_version": spec.loader_version,
                "p_size": packed.size,
                "p_sha256": packed.sha256,
            }),
        )
        .await
        .map_err(cloud)?;
        let pack_id = ticket.get("id").and_then(Value::as_str).unwrap_or("").to_string();
        let path = ticket.get("path").and_then(Value::as_str).unwrap_or("").to_string();
        if !is_uuid(&pack_id) || !social::is_object_path(&path) {
            return fail(t!("Помилка хмари: {}", "Cloud error: {}", "bad_ticket"));
        }
        let bytes = tokio::fs::read(&packed.archive).await?;
        social::upload_object(&state.client, &path, bytes).await.map_err(cloud)?;
        rpc_as(&state.client, &auth, "finish_pack_upload", json!({ "p_pack": pack_id }))
            .await
            .map_err(cloud)?;
        if let Some(old) = ticket.get("old_path").and_then(Value::as_str) {
            if old != path {
                social::delete_object(&state.client, old).await;
            }
        }
        spec.shared_id = pack_id;
        spec.shared_fingerprint = packed.fingerprint.clone();
        custom::save(paths, id, &spec).await?;
        Ok(PublishReport {
            cdn: packed.cdn,
            bundled: packed.bundled,
            skipped: packed.skipped.clone(),
            size_bytes: packed.size,
        })
    }
    .await;
    let _ = tokio::fs::remove_file(&packed.archive).await;
    outcome
}

/// Збірка стала приватною (або видаляється): прибираємо запис і архів із хмари.
pub async fn unpublish(state: &AppState, config: &Config, paths: &Paths, id: &str) -> Result<()> {
    let auth = config.online_account().ok_or_else(offline_account)?;
    let _share = state.social.share_lock.lock().await;
    let result = rpc_as(&state.client, &auth, "unpublish_pack", json!({ "p_local_id": id }))
        .await
        .map_err(cloud)?;
    if let Some(path) = result.get("path").and_then(Value::as_str) {
        social::delete_object(&state.client, path).await;
    }
    if let Some(mut spec) = custom::load(paths, id).await {
        spec.shared_id.clear();
        spec.shared_fingerprint.clear();
        custom::save(paths, id, &spec).await?;
    }
    Ok(())
}

/// Фонове оновлення копії, якщо вміст змінився з моменту публікації (після запуску гри).
pub fn republish_in_background(app: AppHandle, id: String) {
    tauri::async_runtime::spawn(async move {
        let Ok((config, paths)) = load_ctx(&app).await else {
            return;
        };
        if config.online_account().is_none() {
            return;
        }
        let Some(spec) = custom::load(&paths, &id).await else {
            return;
        };
        let state = app.state::<AppState>();
        if !spec.is_shared() {
            // Стала приватною без інтернету — прибираємо копію з хмари, щойно вона доступна.
            if !spec.shared_id.is_empty() {
                let _ = unpublish(&state, &config, &paths, &id).await;
            }
            return;
        }
        if !spec.shared_id.is_empty() && fingerprint(&paths, &id, &spec).await == spec.shared_fingerprint {
            return;
        }
        let _ = publish(&state, &config, &paths, &id).await;
    });
}

/// Хмара з'явилася вперше за сеанс: публікуємо збірки, які стали спільними офлайн
/// або змінилися з моменту останньої публікації.
pub fn republish_all_in_background(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let Ok((_, paths)) = load_ctx(&app).await else {
            return;
        };
        for (id, spec) in custom::list(&paths).await {
            if spec.is_shared() || !spec.shared_id.is_empty() {
                republish_in_background(app.clone(), id);
            }
        }
    });
}

// ───────────────────────────── команди ─────────────────────────────

#[derive(Serialize)]
pub struct VisibilityOutcome {
    pub instance: InstanceView,
    /// Спільну копію опубліковано / оновлено.
    pub published: bool,
    /// Немає хмари: видимість збережено, публікація — щойно з'явиться інтернет.
    pub pending: bool,
    pub error: Option<String>,
    pub report: Option<PublishReport>,
}

/// Видимість власної збірки: `private` | `public` | `on_request`. Для спільних — пакування
/// й публікація (повторний виклик з тим самим значенням оновлює копію для друзів).
#[tauri::command]
pub async fn set_pack_visibility(
    app: AppHandle,
    state: State<'_, AppState>,
    instance_id: String,
    visibility: String,
) -> Result<VisibilityOutcome> {
    if !project::is_safe_id(&instance_id) {
        return fail(tr("Некоректна назва збірки", "Invalid modpack id"));
    }
    if !VISIBILITIES.contains(&visibility.as_str()) {
        return fail("invalid visibility");
    }
    let (config, paths) = load_ctx(&app).await?;
    let mut spec = custom::load(&paths, &instance_id)
        .await
        .ctx(tr("Видимість можна змінити лише для власної збірки", "Visibility can only be changed for custom modpacks"))?;
    spec.visibility = visibility.clone();
    custom::save(&paths, &instance_id, &spec).await?;

    let mut outcome = VisibilityOutcome {
        instance: custom_view(&paths, &instance_id, &spec).await,
        published: false,
        pending: false,
        error: None,
        report: None,
    };
    if config.online_account().is_none() {
        outcome.pending = spec.is_shared() || !spec.shared_id.is_empty();
        return Ok(outcome);
    }
    let result = if spec.is_shared() {
        publish(&state, &config, &paths, &instance_id).await.map(Some)
    } else if !spec.shared_id.is_empty() {
        unpublish(&state, &config, &paths, &instance_id).await.map(|_| None)
    } else {
        Ok(None)
    };
    match result {
        Ok(report) => {
            outcome.published = report.is_some();
            outcome.report = report;
        }
        Err(error) => outcome.error = Some(error.0),
    }
    if let Some(spec) = custom::load(&paths, &instance_id).await {
        outcome.instance = custom_view(&paths, &instance_id, &spec).await;
    }
    Ok(outcome)
}

/// «Завантажити збірку» друга: перевірка доступу → архів зі Storage → нова локальна збірка.
#[tauri::command]
pub async fn download_friend_pack(
    app: AppHandle,
    state: State<'_, AppState>,
    pack_id: String,
) -> Result<ModpackInstalled> {
    if !is_uuid(&pack_id) {
        return fail("invalid id");
    }
    let _guard = state.busy.try_lock().map_err(|_| busy_error())?;
    let (config, paths) = load_ctx(&app).await?;
    let auth = config.online_account().ok_or_else(|| {
        AppError(
            tr(
                "Щоб завантажити збірку друга, потрібне підключення до хмари Terrapolis",
                "Downloading a friend's modpack requires a connection to the Terrapolis cloud",
            )
            .to_string(),
        )
    })?;
    paths.ensure_shared_dirs().await?;
    status(&app, tr("Перевірка доступу до збірки…", "Checking access to the modpack…"));
    let info = rpc_as(&state.client, &auth, "get_pack_download", json!({ "p_pack": pack_id }))
        .await
        .map_err(cloud)?;
    let text = |name: &str| info.get(name).and_then(Value::as_str).unwrap_or("").to_string();
    let path = text("path");
    let sha256 = text("sha256").to_lowercase();
    // Ім'я власника йде в «Звідки збірка» — лише якщо це справжній @username.
    let owner = Some(text("owner_username"))
        .filter(|name| crate::config::validate_username(name).is_ok())
        .unwrap_or_else(|| "friend".to_string());
    let name = text("name");
    let size = info.get("size_bytes").and_then(Value::as_u64).unwrap_or(0);
    if !social::is_object_path(&path)
        || sha256.len() != 64
        || !sha256.chars().all(|c| c.is_ascii_hexdigit())
        || size > MAX_SHARED_PACK_BYTES
    {
        return fail(t!("Помилка хмари: {}", "Cloud error: {}", "bad_pack"));
    }

    status(
        &app,
        &t!("Завантаження збірки «{}» від @{}…", "Downloading \"{}\" from @{}…", name, owner),
    );
    let downloads = paths.cache.join("shared");
    tokio::fs::create_dir_all(&downloads).await?;
    let archive = downloads.join(format!("friend-{pack_id}.zip"));
    download_file(&state.client, &social::public_object_url(&path), &archive, &Checksum::Sha256(sha256), &noop).await?;
    // Хеш збігся з тим, що записав власник, але розмір перевіряємо окремо (захист від підміни запису).
    let downloaded = tokio::fs::metadata(&archive).await.map(|m| m.len()).unwrap_or(u64::MAX);
    if downloaded > MAX_SHARED_PACK_BYTES {
        let _ = tokio::fs::remove_file(&archive).await;
        return fail(t!("Помилка хмари: {}", "Cloud error: {}", "pack_too_large"));
    }
    let origin: String = format!("@{owner} · Terrapolis").chars().take(80).collect();
    let result = modpack::install_shared(&app, &state.client, &paths, &archive, origin, pack_id.clone()).await;
    let _ = tokio::fs::remove_file(&archive).await;
    let report = result?;

    let chosen = report.instance_id.clone();
    update_config(&app, &state, move |config| {
        config.active_instance = chosen;
        Ok(())
    })
    .await?;
    let spec = custom::load(&paths, &report.instance_id).await.ctx("custom.json")?;
    let instance = custom_view(&paths, &report.instance_id, &spec).await;
    Ok(ModpackInstalled { report, instance })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn share_files_require_https_and_hashes() {
        let sha1 = "a".repeat(40);
        assert!(share_file("mods/a.jar", "https://cdn.modrinth.com/a.jar", &Checksum::Sha1(sha1.clone()), 1).is_some());
        assert!(share_file("mods/a.jar", "http://cdn.modrinth.com/a.jar", &Checksum::Sha1(sha1), 1).is_none());
        assert!(share_file("mods/a.jar", "https://cdn.modrinth.com/a.jar", &Checksum::None, 1).is_none());
        assert!(share_file("mods/a.jar", "https://cdn.modrinth.com/a.jar", &Checksum::Sha1("short".into()), 1).is_none());
        let sha1 = "a".repeat(40);
        for bad in ["file:///C:/Windows/evil.jar", "https://127.0.0.1/a.jar", "https://github.com/x/y/a.jar", "https://cdn.modrinth.com@evil.com/a.jar"] {
            assert!(share_file("mods/a.jar", bad, &Checksum::Sha1(sha1.clone()), 1).is_none(), "{bad}");
        }
    }

    #[test]
    fn packing_collects_settings_and_enabled_content_only() {
        let base = std::env::temp_dir().join(format!("tp-share-{}", crate::config::new_id()));
        let make = |rel: &str, bytes: &[u8]| {
            let path = base.join(rel);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, bytes).unwrap();
        };
        make("config/sodium.json", b"{}");
        make("config/deep/a/b.toml", b"x=1");
        make("servers.dat", b"nbt");
        make("options.txt", b"lang:uk_ua");
        make("saves/World/level.dat", b"world");
        make("mods/sodium.jar", b"jar");
        make("mods/off.jar.disabled", b"jar");
        make("resourcepacks/pack.zip", b"zip");
        make(".launcher/custom.json", b"{}");
        let (overrides, content) = collect(&base);
        let names: Vec<&str> = overrides.iter().map(|f| f.relative.as_str()).collect();
        assert_eq!(names, ["config/deep/a/b.toml", "config/sodium.json", "servers.dat"]);
        let names: Vec<&str> = content.iter().map(|f| f.relative.as_str()).collect();
        assert_eq!(names, ["mods/sodium.jar", "resourcepacks/pack.zip"]);

        let spec = CustomSpec {
            name: "X".into(),
            minecraft_version: "1.21.1".into(),
            ..CustomSpec::default()
        };
        let before = fingerprint_of(&spec, &overrides, &content);
        assert_eq!(before, fingerprint_of(&spec, &overrides, &content));
        let after = fingerprint_of(&spec, &overrides, &content[..1]);
        assert_ne!(before, after);

        let archive = base.join("out.zip");
        let manifest = serde_json::to_vec(&ShareManifest { format: SHARE_FORMAT, ..ShareManifest::default() }).unwrap();
        write_archive(&archive, &manifest, &overrides).unwrap();
        let file = std::fs::File::open(&archive).unwrap();
        let zip = zip::ZipArchive::new(file).unwrap();
        let mut names: Vec<String> = zip.file_names().map(str::to_string).collect();
        names.sort();
        assert_eq!(
            names,
            [
                "overrides/config/deep/a/b.toml",
                "overrides/config/sodium.json",
                "overrides/servers.dat",
                SHARE_MANIFEST
            ]
        );
        let _ = std::fs::remove_dir_all(&base);
    }
}
