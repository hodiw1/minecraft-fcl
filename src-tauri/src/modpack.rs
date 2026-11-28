//! Готові збірки спільноти: Modrinth (`.mrpack`) і CurseForge (`.zip` з `manifest.json`).
//!
//! Встановлення створює НОВУ ізольовану власну збірку: версія Minecraft і завантажувач беруться
//! з маніфесту, усі файли (моди, ресурс-паки, шейдери) качаються з CDN платформи з перевіркою
//! хешу, а `overrides/` (config, mods, resourcepacks, shaderpacks…) розпаковуються в папку збірки.
//!
//! Безпека: кожен шлях з архіву чи маніфесту проходить [`safe_pack_path`] (лише відносні
//! «звичайні» компоненти, без `..`, дисків, ADS, імен пристроїв Windows і без службової
//! `.launcher/`), посилання `.mrpack` — лише https на дозволених хостах, хеш обов'язковий.

use std::collections::{HashMap, HashSet};
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};

use futures_util::StreamExt;
use reqwest::Client;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use tauri::AppHandle;

use crate::custom::{self, CustomSpec};
use crate::error::{fail, AppError, Context, Result};
use crate::events::{status, Progress};
use crate::i18n::tr;
use crate::instance;
use crate::loaders::LoaderKind;
use crate::manifest::is_safe_token;
use crate::mods::{self, is_reserved_name, Api, Resolution, ResolvedFile, Source, Target};
use crate::paths::Paths;
use crate::t;
use crate::util::{download_file, noop, Checksum};
use crate::curseforge;
use crate::share::{ShareManifest, SHARE_FORMAT, SHARE_MANIFEST};

/// Скільки байт може займати індекс збірки (`modrinth.index.json` / `manifest.json`).
const MAX_INDEX_BYTES: u64 = 16 * 1024 * 1024;
const MAX_FILES: usize = 5_000;
const MAX_ENTRY_BYTES: u64 = 2 * 1024 * 1024 * 1024;
const MAX_TOTAL_BYTES: u64 = 16 * 1024 * 1024 * 1024;
/// Паралельні завантаження файлів збірки.
const PARALLEL: usize = 6;
/// Хости, з яких специфікація `.mrpack` дозволяє качати файли.
const MRPACK_HOSTS: [&str; 4] = [
    "cdn.modrinth.com",
    "github.com",
    "raw.githubusercontent.com",
    "gitlab.com",
];

#[derive(Serialize, Default, Debug)]
pub struct ModpackReport {
    pub instance_id: String,
    pub name: String,
    pub minecraft_version: String,
    pub loader: String,
    pub loader_version: String,
    /// Завантажено файлів з CDN.
    pub files: usize,
    /// Розпаковано файлів з overrides/.
    pub overrides: usize,
    /// Файли, які не вдалося отримати автоматично (CurseForge: автор заборонив сторонні лаунчери).
    pub missing: Vec<String>,
    /// Пропущено: серверні файли, світи та інший непідтримуваний вміст.
    pub skipped: usize,
}

// ───────────────────────────── безпечні шляхи ─────────────────────────────

/// Відносний шлях усередині збірки з архіву/маніфесту готової збірки.
/// `None` — шлях небезпечний (вихід за межі папки, диск, ADS, ім'я пристрою, `.launcher/`).
pub fn safe_pack_path(raw: &str) -> Option<PathBuf> {
    let normalized = raw.replace('\\', "/");
    if normalized.starts_with('/') || normalized.len() > 400 {
        return None;
    }
    let mut out = PathBuf::new();
    for (index, part) in normalized.split('/').enumerate() {
        let bad = part.is_empty()
            || part == "."
            || part == ".."
            || part.starts_with(' ')
            || part.ends_with(['.', ' '])
            || part.chars().any(|c| c.is_control() || matches!(c, ':' | '<' | '>' | '"' | '|' | '?' | '*'))
            || is_reserved_name(part)
            || (index == 0 && part.eq_ignore_ascii_case(".launcher"));
        if bad {
            return None;
        }
        out.push(part);
    }
    if out.as_os_str().is_empty() || out.components().any(|c| !matches!(c, Component::Normal(_))) {
        return None;
    }
    Some(out)
}

/// https-посилання на одному з дозволених хостів `.mrpack` (без логіна в URL).
fn allowed_mrpack_url(url: &str) -> bool {
    let Some(rest) = url.strip_prefix("https://") else {
        return false;
    };
    let host = rest.split(['/', '?', '#']).next().unwrap_or("");
    !host.contains('@') && MRPACK_HOSTS.iter().any(|allowed| host.eq_ignore_ascii_case(allowed))
}

// ───────────────────────────── формати ─────────────────────────────

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct MrIndex {
    format_version: u32,
    game: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    version_id: String,
    #[serde(default)]
    files: Vec<MrFile>,
    #[serde(default)]
    dependencies: HashMap<String, String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct MrFile {
    path: String,
    #[serde(default)]
    hashes: HashMap<String, String>,
    #[serde(default)]
    env: Option<MrEnv>,
    #[serde(default)]
    downloads: Vec<String>,
    #[serde(default)]
    file_size: u64,
}

#[derive(Deserialize)]
struct MrEnv {
    #[serde(default)]
    client: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CfManifest {
    minecraft: CfMinecraft,
    #[serde(default)]
    manifest_type: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    version: String,
    #[serde(default)]
    files: Vec<CfManifestFile>,
    #[serde(default)]
    overrides: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CfMinecraft {
    version: String,
    #[serde(default)]
    mod_loaders: Vec<CfLoader>,
}

#[derive(Deserialize)]
struct CfLoader {
    id: String,
    #[serde(default)]
    primary: bool,
}

fn yes() -> bool {
    true
}

#[derive(Deserialize)]
struct CfManifestFile {
    #[serde(rename = "projectID")]
    project_id: u64,
    #[serde(rename = "fileID")]
    file_id: u64,
    #[serde(default = "yes")]
    required: bool,
}

/// Версія гри й завантажувач із `dependencies` індексу `.mrpack`.
fn mrpack_loader(deps: &HashMap<String, String>) -> Result<(String, LoaderKind, String)> {
    let mc = deps
        .get("minecraft")
        .map(|v| v.trim().to_string())
        .filter(|v| is_safe_token(v))
        .ctx(tr(
            "У збірці не вказано версію Minecraft",
            "The modpack doesn't specify a Minecraft version",
        ))?;
    let found = [
        ("neoforge", LoaderKind::NeoForge),
        ("forge", LoaderKind::Forge),
        ("fabric-loader", LoaderKind::Fabric),
        ("quilt-loader", LoaderKind::Quilt),
    ]
    .into_iter()
    .find_map(|(key, kind)| deps.get(key).map(|version| (kind, version.trim().to_string())));
    match found {
        Some((kind, version)) if is_safe_token(&version) => Ok((mc, kind, version)),
        Some(_) => fail(tr(
            "Некоректна версія завантажувача в збірці",
            "Invalid loader version in the modpack",
        )),
        None => Ok((mc, LoaderKind::Vanilla, String::new())),
    }
}

/// `forge-47.2.0` / `neoforge-21.1.77` / `fabric-0.16.5` → (тип, версія).
fn cf_loader(minecraft: &CfMinecraft) -> Result<(LoaderKind, String)> {
    let Some(loader) = minecraft
        .mod_loaders
        .iter()
        .find(|l| l.primary)
        .or_else(|| minecraft.mod_loaders.first())
    else {
        return Ok((LoaderKind::Vanilla, String::new()));
    };
    let (kind, version) = loader.id.trim().split_once('-').ctx(tr(
        "Некоректний завантажувач у збірці",
        "Invalid loader in the modpack",
    ))?;
    let kind = LoaderKind::parse(kind)?;
    // Деякі збірки NeoForge 1.20.1 пишуть «neoforge-1.20.1-47.1.84».
    let version = version
        .strip_prefix(&format!("{}-", minecraft.version.trim()))
        .unwrap_or(version)
        .to_string();
    if !is_safe_token(&version) {
        return fail(tr(
            "Некоректна версія завантажувача в збірці",
            "Invalid loader version in the modpack",
        ));
    }
    Ok((kind, version))
}

/// Папка для файлу CurseForge за `classId` проєкту.
fn cf_folder(class_id: Option<u64>, filename: &str) -> Option<&'static str> {
    match class_id {
        Some(6) => Some("mods"),
        Some(12) => Some("resourcepacks"),
        Some(6552) => Some("shaderpacks"),
        Some(_) => None,
        None if filename.to_ascii_lowercase().ends_with(".jar") => Some("mods"),
        None => None,
    }
}

// ───────────────────────────── архів ─────────────────────────────

fn open_zip(path: &Path) -> Result<zip::ZipArchive<std::fs::File>> {
    let file = std::fs::File::open(path).ctx(tr("Не вдалося відкрити архів збірки", "Couldn't open the modpack archive"))?;
    zip::ZipArchive::new(file).ctx(tr("Архів збірки пошкоджений", "The modpack archive is damaged"))
}

fn read_json<T: DeserializeOwned>(archive: &Path, name: &str) -> Result<T> {
    let mut zip = open_zip(archive)?;
    let entry = zip.by_name(name).map_err(|_| {
        AppError(t!(
            "В архіві збірки немає {}",
            "The modpack archive has no {}",
            name
        ))
    })?;
    if entry.size() > MAX_INDEX_BYTES {
        return fail(t!("{} завеликий", "{} is too large", name));
    }
    let mut text = String::new();
    entry.take(MAX_INDEX_BYTES).read_to_string(&mut text).ctx(name)?;
    serde_json::from_str(text.trim_start_matches('\u{feff}'))
        .map_err(|error| AppError(t!("{} пошкоджений: {}", "{} is invalid: {}", name, error)))
}

/// Обмеження розпакування overrides/: розмір файлу, сумарний обсяг (за ФАКТИЧНО розпакованими
/// байтами — заголовки zip можуть брехати) і, для збірок друзів, білий список тек верхнього рівня.
#[derive(Clone, Copy, Debug)]
struct ExtractLimits {
    entry: u64,
    total: u64,
    files: usize,
    top: Option<&'static [&'static str]>,
}

const MODPACK_LIMITS: ExtractLimits = ExtractLimits {
    entry: MAX_ENTRY_BYTES,
    total: MAX_TOTAL_BYTES,
    files: 100_000,
    top: None,
};

/// Пакет друга: архів до 45 МБ, тож розпакованих даних не може бути гігабайти (zip-bomb),
/// а писати можна лише в налаштування й теки контенту — не в saves/, screenshots/, logs/ тощо.
const SHARED_TOP: [&str; 12] = [
    "config", "defaultconfigs", "kubejs", "scripts", "global_packs", "openloader",
    "patchouli_books", "mods", "resourcepacks", "shaderpacks", "servers.dat", "icon.png",
];
const SHARED_LIMITS: ExtractLimits = ExtractLimits {
    entry: 256 * 1024 * 1024,
    total: 1024 * 1024 * 1024,
    files: 6_000,
    top: Some(&SHARED_TOP),
};

/// Розпаковує `prefixes` (по черзі, пізніші перекривають ранні) у `dest`. Повертає кількість файлів.
fn extract_overrides(archive: &Path, prefixes: &[String], dest: &Path, limits: ExtractLimits) -> Result<(usize, usize)> {
    let mut zip = open_zip(archive)?;
    let mut written = 0usize;
    let mut skipped = 0usize;
    let mut total = 0u64;
    let mut buffer = vec![0u8; 65536];
    for prefix in prefixes {
        for index in 0..zip.len() {
            let mut entry = zip.by_index(index)?;
            if entry.is_dir() {
                continue;
            }
            let name = entry.name().replace('\\', "/");
            let Some(relative) = name.strip_prefix(prefix.as_str()) else {
                continue;
            };
            // Подвійна перевірка: власні правила + перевірка бібліотеки zip.
            let Some(safe) = safe_pack_path(relative).filter(|_| entry.enclosed_name().is_some()) else {
                skipped += 1;
                continue;
            };
            let top = relative.split('/').next().unwrap_or("");
            if let Some(allowed) = limits.top {
                if !allowed.iter().any(|dir| dir.eq_ignore_ascii_case(top)) {
                    skipped += 1;
                    continue;
                }
            }
            if entry.size() > limits.entry {
                return fail(t!("Файл у збірці завеликий: {}", "A file in the modpack is too large: {}", relative));
            }
            if written >= limits.files {
                return fail(tr("У збірці забагато файлів", "The modpack has too many files"));
            }
            let target = dest.join(&safe);
            if target.is_dir() {
                skipped += 1;
                continue;
            }
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let mut temp = target.clone().into_os_string();
            temp.push(".part");
            let temp = PathBuf::from(temp);
            let mut out = std::fs::File::create(&temp)?;
            let mut copied = 0u64;
            loop {
                let read = entry.read(&mut buffer)?;
                if read == 0 {
                    break;
                }
                copied += read as u64;
                total += read as u64;
                if copied > limits.entry || total > limits.total {
                    drop(out);
                    let _ = std::fs::remove_file(&temp);
                    return fail(tr("Архів збірки пошкоджений або завеликий", "The modpack archive is damaged or too large"));
                }
                out.write_all(&buffer[..read])?;
            }
            out.flush()?;
            drop(out);
            std::fs::rename(&temp, &target)?;
            written += 1;
        }
    }
    Ok((written, skipped))
}

// ───────────────────────────── завантаження ─────────────────────────────

struct Job {
    relative: PathBuf,
    urls: Vec<String>,
    checksum: Checksum,
    label: String,
}

async fn download_all(app: &AppHandle, client: &Client, dir: &Path, jobs: Vec<Job>, total: u64) -> Result<usize> {
    let count = jobs.len();
    let progress = Progress::new(app, "modpack", total.max(1));
    let progress = &progress;
    let results: Vec<Result<()>> = futures_util::stream::iter(jobs.into_iter().map(|job| async move {
        progress.set_file(&job.label);
        let dest = dir.join(&job.relative);
        let mut last = String::new();
        for url in &job.urls {
            match download_file(client, url, &dest, &job.checksum, &|bytes| progress.add(bytes)).await {
                Ok(()) => return Ok(()),
                Err(error) => last = error.0,
            }
        }
        fail(format!("{}: {last}", job.label))
    }))
    .buffer_unordered(PARALLEL)
    .collect()
    .await;
    for result in results {
        result?;
    }
    progress.finish();
    Ok(count)
}

/// Готова до встановлення збірка: що створити і що завантажити.
struct Plan {
    spec: CustomSpec,
    jobs: Vec<Job>,
    total: u64,
    overrides: Vec<String>,
    missing: Vec<String>,
    skipped: usize,
    limits: ExtractLimits,
}

fn plan_mrpack(index: MrIndex, file: &ResolvedFile) -> Result<Plan> {
    if index.game != "minecraft" || index.format_version != 1 {
        return fail(tr(
            "Непідтримуваний формат .mrpack",
            "Unsupported .mrpack format",
        ));
    }
    if index.files.len() > MAX_FILES {
        return fail(tr("У збірці забагато файлів", "The modpack has too many files"));
    }
    let (mc, kind, loader_version) = mrpack_loader(&index.dependencies)?;
    let mut jobs = Vec::new();
    let mut seen = HashSet::new();
    let mut skipped = 0usize;
    let mut total = 0u64;
    for entry in index.files {
        if entry.env.as_ref().is_some_and(|env| env.client == "unsupported") {
            skipped += 1;
            continue;
        }
        let relative = safe_pack_path(&entry.path).ok_or_else(|| {
            AppError(t!(
                "Небезпечний шлях у збірці: {}",
                "Unsafe path in the modpack: {}",
                entry.path
            ))
        })?;
        if !seen.insert(entry.path.to_lowercase()) {
            continue;
        }
        let urls: Vec<String> = entry.downloads.into_iter().filter(|u| allowed_mrpack_url(u)).collect();
        if urls.is_empty() {
            return fail(t!(
                "Файл «{}» не має дозволеного джерела завантаження",
                "File \"{}\" has no allowed download source",
                entry.path
            ));
        }
        let checksum = match (entry.hashes.get("sha512"), entry.hashes.get("sha1")) {
            (Some(hash), _) if hash.len() == 128 && hash.chars().all(|c| c.is_ascii_hexdigit()) => Checksum::Sha512(hash.clone()),
            (_, Some(hash)) if hash.len() == 40 && hash.chars().all(|c| c.is_ascii_hexdigit()) => Checksum::Sha1(hash.clone()),
            _ => {
                return fail(t!(
                    "Для файлу «{}» немає контрольної суми",
                    "File \"{}\" has no checksum",
                    entry.path
                ))
            }
        };
        total += entry.file_size;
        let label = entry.path.rsplit('/').next().unwrap_or(&entry.path).to_string();
        jobs.push(Job {
            relative,
            urls,
            checksum,
            label,
        });
    }
    let name = if index.name.trim().is_empty() { file.title.clone() } else { index.name.trim().to_string() };
    let version = if index.version_id.trim().is_empty() { file.version_number.clone() } else { index.version_id.trim().to_string() };
    Ok(Plan {
        spec: CustomSpec {
            name,
            minecraft_version: mc,
            loader: kind.id().to_string(),
            loader_version,
            created: 0,
            origin: format!("Modrinth · {version}"),
            ..CustomSpec::default()
        },
        jobs,
        total,
        overrides: vec!["overrides/".to_string(), "client-overrides/".to_string()],
        missing: Vec::new(),
        skipped,
        limits: MODPACK_LIMITS,
    })
}

async fn plan_curseforge(api: &Api<'_>, manifest: CfManifest, file: &ResolvedFile) -> Result<Plan> {
    if !manifest.manifest_type.is_empty() && manifest.manifest_type != "minecraftModpack" {
        return fail(tr(
            "Це не збірка Minecraft CurseForge",
            "This is not a CurseForge Minecraft modpack",
        ));
    }
    let mc = manifest.minecraft.version.trim().to_string();
    if !is_safe_token(&mc) {
        return fail(tr(
            "У збірці не вказано версію Minecraft",
            "The modpack doesn't specify a Minecraft version",
        ));
    }
    let (kind, loader_version) = cf_loader(&manifest.minecraft)?;
    let ids: Vec<u64> = manifest.files.iter().filter(|f| f.required).map(|f| f.file_id).collect();
    if ids.len() > MAX_FILES {
        return fail(tr("У збірці забагато файлів", "The modpack has too many files"));
    }
    let wanted: HashSet<u64> = manifest.files.iter().map(|f| f.project_id).collect();
    let infos = if ids.is_empty() { Vec::new() } else { curseforge::pack_files(api, &ids).await? };
    let mut jobs = Vec::new();
    let mut missing = Vec::new();
    let mut skipped = manifest.files.iter().filter(|f| !f.required).count();
    let mut seen = HashSet::new();
    let mut total = 0u64;
    for info in infos {
        if !wanted.contains(&info.mod_id) {
            continue;
        }
        let Some(folder) = cf_folder(info.class_id, &info.filename) else {
            skipped += 1;
            continue;
        };
        if info.filename.contains(['/', '\\']) {
            skipped += 1;
            continue;
        }
        let Some(relative) = safe_pack_path(&format!("{folder}/{}", info.filename)) else {
            skipped += 1;
            continue;
        };
        if !seen.insert(relative.clone()) {
            continue;
        }
        let Some(url) = info.url.clone() else {
            missing.push(format!("{} ({})", info.title, info.filename));
            continue;
        };
        total += info.size;
        jobs.push(Job {
            relative,
            urls: vec![url],
            checksum: info.checksum.clone(),
            label: info.filename.clone(),
        });
    }
    let overrides = manifest
        .overrides
        .as_deref()
        .map(|o| o.trim().trim_matches('/').to_string())
        .filter(|o| !o.is_empty())
        .unwrap_or_else(|| "overrides".to_string());
    let overrides = match safe_pack_path(&overrides) {
        Some(_) => vec![format!("{overrides}/")],
        None => Vec::new(),
    };
    let name = if manifest.name.trim().is_empty() { file.title.clone() } else { manifest.name.trim().to_string() };
    let version = if manifest.version.trim().is_empty() { file.version_number.clone() } else { manifest.version.trim().to_string() };
    Ok(Plan {
        spec: CustomSpec {
            name,
            minecraft_version: mc,
            loader: kind.id().to_string(),
            loader_version,
            created: 0,
            origin: format!("CurseForge · {version}"),
            ..CustomSpec::default()
        },
        jobs,
        total,
        overrides,
        missing,
        skipped,
        limits: MODPACK_LIMITS,
    })
}

/// Встановлює готову збірку як нову власну збірку. Повертає звіт з id нової збірки.
pub async fn install(
    app: &AppHandle,
    api: &Api<'_>,
    paths: &Paths,
    project: &str,
    version: Option<&str>,
) -> Result<ModpackReport> {
    status(app, tr("Отримання інформації про збірку…", "Fetching modpack info…"));
    let file = match mods::resolve(api, &Target::modpacks(), project, version).await? {
        Resolution::File(file) => file,
        Resolution::NoVersion { title } => {
            return fail(t!(
                "У збірки «{}» немає файлів для встановлення",
                "\"{}\" has no installable files",
                title
            ))
        }
    };
    if !file.url.starts_with("https://") {
        return fail("modpack url is not https");
    }
    let cache = paths.cache.join("modpacks");
    tokio::fs::create_dir_all(&cache).await?;
    let safe_version: String = file
        .version_id
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'))
        .take(64)
        .collect();
    let extension = if api.source == Source::Modrinth { "mrpack" } else { "zip" };
    let archive = cache.join(format!("{}-{safe_version}.{extension}", api.source.id()));

    status(
        app,
        &t!("Завантаження «{}»…", "Downloading \"{}\"…", file.title),
    );
    download_file(api.client, &file.url, &archive, &file.checksum, &noop).await?;
    let result = install_archive(app, api, paths, &archive, &file).await;
    let _ = tokio::fs::remove_file(&archive).await;
    result
}

async fn install_archive(
    app: &AppHandle,
    api: &Api<'_>,
    paths: &Paths,
    archive: &Path,
    file: &ResolvedFile,
) -> Result<ModpackReport> {
    status(app, tr("Перевірка вмісту збірки…", "Checking the modpack contents…"));
    let plan = match api.source {
        Source::Modrinth => {
            let path = archive.to_path_buf();
            let index: MrIndex = tokio::task::spawn_blocking(move || read_json(&path, "modrinth.index.json")).await??;
            plan_mrpack(index, file)?
        }
        Source::CurseForge => {
            let path = archive.to_path_buf();
            let manifest: CfManifest = tokio::task::spawn_blocking(move || read_json(&path, "manifest.json")).await??;
            plan_curseforge(api, manifest, file).await?
        }
    };
    run_plan(app, api.client, paths, archive, plan).await
}

/// Спільний хвіст встановлення: нова власна збірка, файли з CDN, overrides/ з архіву.
/// Будь-яка помилка прибирає напівстворену збірку.
async fn run_plan(app: &AppHandle, client: &Client, paths: &Paths, archive: &Path, plan: Plan) -> Result<ModpackReport> {
    let Plan {
        spec,
        jobs,
        total,
        overrides,
        missing,
        skipped,
        limits,
    } = plan;
    let mut report = ModpackReport {
        name: spec.name.clone(),
        minecraft_version: spec.minecraft_version.clone(),
        loader: spec.loader.clone(),
        loader_version: spec.loader_version.clone(),
        missing,
        skipped,
        ..ModpackReport::default()
    };
    // Збірка створюється лише після повної перевірки маніфесту: битий архів не лишає «сміття».
    let id = custom::create(paths, spec).await?;
    report.instance_id = id.clone();
    let dir = paths.instance_dir(&id);

    let outcome: Result<()> = async {
        instance::ensure_dirs(paths, &id).await?;
        status(
            app,
            &t!(
                "Завантаження файлів збірки: {}…",
                "Downloading modpack files: {}…",
                jobs.len()
            ),
        );
        report.files = download_all(app, client, &dir, jobs, total).await?;
        status(app, tr("Розпаковування overrides…", "Extracting overrides…"));
        let (source, target) = (archive.to_path_buf(), dir.clone());
        let (written, skipped) =
            tokio::task::spawn_blocking(move || extract_overrides(&source, &overrides, &target, limits)).await??;
        report.overrides = written;
        report.skipped += skipped;
        Ok(())
    }
    .await;
    if let Err(error) = outcome {
        let _ = instance::delete(paths, &id).await;
        return Err(error);
    }
    status(
        app,
        &t!(
            "Збірку «{}» встановлено: файлів — {}",
            "Modpack \"{}\" installed: {} files",
            report.name,
            report.files + report.overrides
        ),
    );
    Ok(report)
}

// ───────────────────────────── збірка друга (Terrapolis) ─────────────────────────────

/// Хости, з яких пакет друга може качати файли: CDN Modrinth і CurseForge.
/// Лише CDN Modrinth і CurseForge: жодних github.com/довільних хостів, file://, localhost.
const SHARE_HOSTS: [&str; 3] = ["cdn.modrinth.com", "edge.forgecdn.net", "mediafilez.forgecdn.net"];

pub(crate) fn allowed_share_url(url: &str) -> bool {
    let Some(rest) = url.strip_prefix("https://") else {
        return false;
    };
    let host = rest.split(['/', '?', '#']).next().unwrap_or("");
    !host.contains('@') && SHARE_HOSTS.iter().any(|allowed| host.eq_ignore_ascii_case(allowed))
}

fn is_hex(value: &str, len: usize) -> bool {
    value.len() == len && value.chars().all(|c| c.is_ascii_hexdigit())
}

fn plan_shared(manifest: &ShareManifest, origin: String, shared_from: String) -> Result<Plan> {
    if manifest.format != SHARE_FORMAT {
        return fail(tr(
            "Збірку друга створено новішою версією лаунчера — онови Terrapolis",
            "Your friend's modpack was made by a newer launcher — update Terrapolis",
        ));
    }
    if manifest.files.len() > MAX_FILES {
        return fail(tr("У збірці забагато файлів", "The modpack has too many files"));
    }
    let kind = LoaderKind::parse(&manifest.loader)?;
    let mc = manifest.minecraft_version.trim().to_string();
    let loader_version = manifest.loader_version.trim().to_string();
    if !is_safe_token(&mc) || (kind != LoaderKind::Vanilla && !is_safe_token(&loader_version)) {
        return fail(tr("Некоректна версія гри у збірці", "Invalid game version in the modpack"));
    }
    let mut jobs = Vec::new();
    let mut seen = HashSet::new();
    let mut total = 0u64;
    for file in &manifest.files {
        let relative = safe_pack_path(&file.path).ok_or_else(|| {
            AppError(t!("Небезпечний шлях у збірці: {}", "Unsafe path in the modpack: {}", file.path))
        })?;
        let top = file.path.split('/').next().unwrap_or("");
        if !matches!(top, "mods" | "resourcepacks" | "shaderpacks") {
            return fail(t!("Небезпечний шлях у збірці: {}", "Unsafe path in the modpack: {}", file.path));
        }
        if !seen.insert(file.path.to_ascii_lowercase()) {
            continue;
        }
        if !allowed_share_url(&file.url) {
            return fail(t!(
                "Файл «{}» не має дозволеного джерела завантаження",
                "File \"{}\" has no allowed download source",
                file.path
            ));
        }
        let checksum = if is_hex(&file.sha512, 128) {
            Checksum::Sha512(file.sha512.clone())
        } else if is_hex(&file.sha1, 40) {
            Checksum::Sha1(file.sha1.clone())
        } else {
            return fail(t!("Для файлу «{}» немає контрольної суми", "File \"{}\" has no checksum", file.path));
        };
        total += file.size.min(MAX_ENTRY_BYTES);
        jobs.push(Job {
            relative,
            urls: vec![file.url.clone()],
            checksum,
            label: file.path.rsplit('/').next().unwrap_or(&file.path).to_string(),
        });
    }
    let name: String = manifest.name.trim().chars().filter(|c| !c.is_control()).take(40).collect();
    Ok(Plan {
        spec: CustomSpec {
            name: if name.is_empty() { "Terrapolis".to_string() } else { name },
            minecraft_version: mc,
            loader: kind.id().to_string(),
            loader_version,
            origin,
            shared_from,
            ..CustomSpec::default()
        },
        jobs,
        total,
        overrides: vec!["overrides/".to_string()],
        missing: Vec::new(),
        skipped: 0,
        limits: SHARED_LIMITS,
    })
}

/// Встановлює збірку друга (пакет Terrapolis) як нову власну збірку й відновлює записи
/// менеджера контенту (назви, іконки, версії модів).
pub async fn install_shared(
    app: &AppHandle,
    client: &Client,
    paths: &Paths,
    archive: &Path,
    origin: String,
    shared_from: String,
) -> Result<ModpackReport> {
    status(app, tr("Перевірка вмісту збірки…", "Checking the modpack contents…"));
    let path = archive.to_path_buf();
    let mut manifest: ShareManifest = tokio::task::spawn_blocking(move || read_json(&path, SHARE_MANIFEST)).await??;
    let plan = plan_shared(&manifest, origin, shared_from)?;
    let report = run_plan(app, client, paths, archive, plan).await?;
    // Іконка друга: лише справжній PNG, і лише в .launcher/icon.png (не в корені гри).
    let dropped = paths.instance_dir(&report.instance_id).join(custom::ICON_FILE);
    if let Ok(bytes) = tokio::fs::read(&dropped).await {
        let _ = tokio::fs::remove_file(&dropped).await;
        if custom::check_icon(&bytes).is_ok() {
            let _ = custom::save_icon(paths, &report.instance_id, Some(&bytes)).await;
        }
    }
    for kind in [mods::ContentKind::Mod, mods::ContentKind::ResourcePack, mods::ContentKind::Shader] {
        let dir = mods::content_dir(paths, &report.instance_id, kind);
        let records: Vec<mods::ModRecord> = std::mem::take(manifest.records_mut(kind))
            .into_iter()
            .filter(|r| {
                safe_pack_path(&r.filename).is_some_and(|p| p.components().count() == 1)
                    && dir.join(&r.filename).is_file()
            })
            .collect();
        if !records.is_empty() {
            let _ = mods::save_records(paths, &report.instance_id, kind, &records).await;
        }
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn zip_with(path: &Path, files: &[(&str, &[u8])]) {
        use zip::write::SimpleFileOptions;
        let mut zip = zip::ZipWriter::new(std::fs::File::create(path).unwrap());
        for (name, bytes) in files {
            zip.start_file(*name, SimpleFileOptions::default()).unwrap();
            zip.write_all(bytes).unwrap();
        }
        zip.finish().unwrap();
    }

    #[test]
    fn shared_overrides_never_touch_personal_folders() {
        let base = std::env::temp_dir().join(format!("tp-extract-{}", crate::config::new_id()));
        std::fs::create_dir_all(&base).unwrap();
        let archive = base.join("pack.zip");
        zip_with(
            &archive,
            &[
                ("overrides/config/sodium.json", b"{}"),
                ("overrides/servers.dat", b"nbt"),
                ("overrides/saves/World/level.dat", b"evil"),
                ("overrides/screenshots/x.png", b"evil"),
                ("overrides/options.txt", b"evil"),
                ("overrides/.launcher/custom.json", b"evil"),
                ("overrides/../escape.txt", b"evil"),
            ],
        );
        let dest = base.join("instance");
        std::fs::create_dir_all(&dest).unwrap();
        let (written, skipped) = extract_overrides(&archive, &["overrides/".to_string()], &dest, SHARED_LIMITS).unwrap();
        assert_eq!(written, 2);
        assert_eq!(skipped, 5);
        assert!(dest.join("config/sodium.json").is_file());
        assert!(dest.join("servers.dat").is_file());
        assert!(!dest.join("saves").exists());
        assert!(!dest.join("screenshots").exists());
        assert!(!dest.join("options.txt").exists());
        assert!(!dest.join(".launcher").exists());
        assert!(!base.join("escape.txt").exists());

        // Сумарний ліміт рахується за фактично розпакованими байтами.
        let tiny = ExtractLimits { entry: 1024, total: 15, files: 100, top: None };
        let bomb = base.join("bomb.zip");
        zip_with(&bomb, &[("overrides/a.txt", &[b'x'; 10]), ("overrides/b.txt", &[b'y'; 10])]);
        assert!(extract_overrides(&bomb, &["overrides/".to_string()], &base.join("bomb"), tiny).is_err());
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn shared_urls_are_limited_to_mod_cdns() {
        assert!(allowed_share_url("https://cdn.modrinth.com/data/x/versions/y/a.jar"));
        assert!(allowed_share_url("https://edge.forgecdn.net/files/1/2/a.jar"));
        for bad in [
            "file:///C:/Windows/System32/evil.jar",
            "http://cdn.modrinth.com/a.jar",
            "https://127.0.0.1/a.jar",
            "https://localhost/a.jar",
            "https://github.com/evil/repo/releases/download/v1/a.jar",
            "https://cdn.modrinth.com.evil.com/a.jar",
            "https://user@cdn.modrinth.com/a.jar",
        ] {
            assert!(!allowed_share_url(bad), "{bad}");
        }
    }

    #[test]
    fn shared_plan_validates_paths_hosts_and_hashes() {
        let sha1 = "b".repeat(40);
        let good = ShareManifest {
            format: SHARE_FORMAT,
            name: "Friends SMP".into(),
            minecraft_version: "1.21.1".into(),
            loader: "fabric".into(),
            loader_version: "0.16.5".into(),
            files: vec![crate::share::ShareFile {
                path: "mods/sodium.jar".into(),
                url: "https://cdn.modrinth.com/data/AANobbMI/versions/x/sodium.jar".into(),
                sha1: sha1.clone(),
                sha512: String::new(),
                size: 10,
            }],
            ..ShareManifest::default()
        };
        let plan = plan_shared(&good, "@friend · Terrapolis".into(), String::new()).unwrap();
        assert_eq!(plan.jobs.len(), 1);
        assert_eq!(plan.spec.loader, "fabric");
        assert_eq!(plan.overrides, ["overrides/"]);

        let mut evil = good.clone();
        evil.files[0].path = "../../evil.jar".into();
        assert!(plan_shared(&evil, String::new(), String::new()).is_err());
        let mut evil = good.clone();
        evil.files[0].path = "config/evil.jar".into();
        assert!(plan_shared(&evil, String::new(), String::new()).is_err());
        let mut evil = good.clone();
        evil.files[0].url = "https://evil.example.com/x.jar".into();
        assert!(plan_shared(&evil, String::new(), String::new()).is_err());
        let mut evil = good.clone();
        evil.files[0].sha1 = String::new();
        assert!(plan_shared(&evil, String::new(), String::new()).is_err());
        let mut newer = good;
        newer.format = 99;
        assert!(plan_shared(&newer, String::new(), String::new()).is_err());
    }

    #[test]
    fn pack_paths_never_escape_the_instance() {
        for ok in ["mods/sodium.jar", "config/a/b.toml", "options.txt", "kubejs/server_scripts/x.js", "resourcepacks/Pack 1.zip"] {
            assert!(safe_pack_path(ok).is_some(), "{ok}");
        }
        for bad in [
            "../evil.jar",
            "mods/../../evil.jar",
            "/abs/path",
            "\\\\server\\share\\x",
            "C:/Windows/x.dll",
            "C:x",
            "mods/x.jar:stream",
            ".launcher/custom.json",
            ".LAUNCHER/state.json",
            "mods//x.jar",
            "mods/./x.jar",
            "mods/CON",
            "mods/x.jar.",
            "",
        ] {
            assert!(safe_pack_path(bad).is_none(), "{bad}");
        }
    }

    #[test]
    fn mrpack_hosts_are_whitelisted() {
        assert!(allowed_mrpack_url("https://cdn.modrinth.com/data/AANobbMI/versions/x/sodium.jar"));
        assert!(allowed_mrpack_url("https://github.com/o/r/releases/download/v1/x.jar"));
        assert!(!allowed_mrpack_url("http://cdn.modrinth.com/x.jar"));
        assert!(!allowed_mrpack_url("https://evil.example.com/x.jar"));
        assert!(!allowed_mrpack_url("https://cdn.modrinth.com@evil.example.com/x.jar"));
        assert!(!allowed_mrpack_url("https://cdn.modrinth.com.evil.example.com/x.jar"));
    }

    #[test]
    fn loaders_are_parsed() {
        let mut deps = HashMap::new();
        deps.insert("minecraft".to_string(), "1.21.1".to_string());
        deps.insert("fabric-loader".to_string(), "0.16.5".to_string());
        let (mc, kind, version) = mrpack_loader(&deps).unwrap();
        assert_eq!((mc.as_str(), kind, version.as_str()), ("1.21.1", LoaderKind::Fabric, "0.16.5"));

        let minecraft = CfMinecraft {
            version: "1.20.1".into(),
            mod_loaders: vec![CfLoader { id: "neoforge-1.20.1-47.1.84".into(), primary: true }],
        };
        let (kind, version) = cf_loader(&minecraft).unwrap();
        assert_eq!((kind, version.as_str()), (LoaderKind::NeoForge, "47.1.84"));

        let minecraft = CfMinecraft {
            version: "1.20.1".into(),
            mod_loaders: vec![CfLoader { id: "forge-47.2.0".into(), primary: true }],
        };
        assert_eq!(cf_loader(&minecraft).unwrap(), (LoaderKind::Forge, "47.2.0".to_string()));
        assert_eq!(cf_folder(Some(12), "x.zip"), Some("resourcepacks"));
        assert_eq!(cf_folder(Some(17), "world.zip"), None);
    }

    #[test]
    fn mrpack_plan_rejects_traversal_and_missing_hashes() {
        let file = ResolvedFile {
            project_id: "p".into(),
            title: "Pack".into(),
            icon_url: None,
            version_id: "v".into(),
            version_number: "1.0".into(),
            url: "https://cdn.modrinth.com/x.mrpack".into(),
            filename: "x.mrpack".into(),
            checksum: Checksum::None,
            dependencies: Vec::new(),
        };
        let parse = |json: &str| plan_mrpack(serde_json::from_str(json).unwrap(), &file);
        let sha = "a".repeat(128);
        let good = format!(
            r#"{{"formatVersion":1,"game":"minecraft","name":"P","files":[{{"path":"mods/a.jar","hashes":{{"sha512":"{sha}"}},"downloads":["https://cdn.modrinth.com/a.jar"],"fileSize":10}},{{"path":"mods/server.jar","env":{{"client":"unsupported"}},"hashes":{{"sha512":"{sha}"}},"downloads":["https://cdn.modrinth.com/s.jar"]}}],"dependencies":{{"minecraft":"1.21.1","fabric-loader":"0.16.5"}}}}"#
        );
        let plan = parse(&good).unwrap();
        assert_eq!(plan.jobs.len(), 1);
        assert_eq!(plan.skipped, 1);
        assert_eq!(plan.spec.loader, "fabric");

        let evil = good.replace("mods/a.jar", "../../a.jar");
        assert!(parse(&evil).is_err());
        let no_hash = good.replacen(&format!(r#""sha512":"{sha}""#), r#""md5":"x""#, 1);
        assert!(parse(&no_hash).is_err());
        let bad_host = good.replacen("https://cdn.modrinth.com/a.jar", "https://evil.example.com/a.jar", 1);
        assert!(parse(&bad_host).is_err());
    }
}
