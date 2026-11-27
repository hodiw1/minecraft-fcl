//! Менеджер контенту у стилі Prism Launcher: пошук і встановлення модів, ресурс-паків,
//! шейдерпаків і готових збірок з Modrinth та CurseForge (з фільтром за версією Minecraft
//! і завантажувачем, вибором версії файлу, перевіркою SHA-1/SHA-512 і обов'язковими
//! залежностями модів), вмикання/вимикання (`.jar` ↔ `.jar.disabled`, `.zip` ↔ `.zip.disabled`)
//! та видалення.

use std::collections::HashSet;
use std::path::PathBuf;

use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::constants::ENABLE_CURSEFORGE;
use crate::error::{fail, Result};
use crate::i18n::tr;
use crate::manifest::{is_mod_path, Manifest};
use crate::paths::Paths;
use crate::t;
use crate::util::{download_file, noop, Checksum};
use crate::{curseforge, modrinth};

pub const PAGE: u32 = 20;
const MAX_INSTALLS: usize = 40;

/// Тип контенту в каталозі. Модпак — окремий випадок: він не кладеться в папку збірки,
/// а створює нову (див. `modpack.rs`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ContentKind {
    Mod,
    ResourcePack,
    Shader,
    Modpack,
}

impl ContentKind {
    pub fn parse(raw: &str) -> Result<Self> {
        match raw.trim().to_lowercase().as_str() {
            "" | "mod" | "mods" => Ok(ContentKind::Mod),
            "resourcepack" | "resourcepacks" => Ok(ContentKind::ResourcePack),
            "shader" | "shaders" | "shaderpack" | "shaderpacks" => Ok(ContentKind::Shader),
            "modpack" | "modpacks" => Ok(ContentKind::Modpack),
            _ => fail("unknown content type"),
        }
    }

    /// Папка всередині збірки.
    pub fn folder(self) -> &'static str {
        match self {
            ContentKind::Mod => "mods",
            ContentKind::ResourcePack => "resourcepacks",
            ContentKind::Shader => "shaderpacks",
            ContentKind::Modpack => "",
        }
    }

    /// Розширення файлу (у нижньому регістрі, з крапкою).
    pub fn extension(self) -> &'static str {
        match self {
            ContentKind::Mod => ".jar",
            ContentKind::ResourcePack | ContentKind::Shader => ".zip",
            ContentKind::Modpack => "",
        }
    }

    /// Файл записів лаунчера в `.launcher/` збірки.
    fn records_name(self) -> &'static str {
        match self {
            ContentKind::Mod => "mods.json",
            ContentKind::ResourcePack => "resourcepacks.json",
            ContentKind::Shader => "shaderpacks.json",
            ContentKind::Modpack => "modpacks.json",
        }
    }

    pub fn modrinth_type(self) -> &'static str {
        match self {
            ContentKind::Mod => "mod",
            ContentKind::ResourcePack => "resourcepack",
            ContentKind::Shader => "shader",
            ContentKind::Modpack => "modpack",
        }
    }

    /// `classId` у CurseForge API.
    pub fn curseforge_class(self) -> u64 {
        match self {
            ContentKind::Mod => 6,
            ContentKind::ResourcePack => 12,
            ContentKind::Shader => 6552,
            ContentKind::Modpack => 4471,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Source {
    Modrinth,
    CurseForge,
}

impl Source {
    pub fn parse(raw: &str) -> Result<Self> {
        match raw.trim().to_lowercase().as_str() {
            "" | "modrinth" => Ok(Source::Modrinth),
            "curseforge" if ENABLE_CURSEFORGE => Ok(Source::CurseForge),
            "curseforge" => fail(tr(
                "CurseForge тимчасово вимкнено — шукай моди на Modrinth",
                "CurseForge is temporarily disabled — search mods on Modrinth",
            )),
            _ => fail("unknown mod source"),
        }
    }

    pub fn id(self) -> &'static str {
        match self {
            Source::Modrinth => "modrinth",
            Source::CurseForge => "curseforge",
        }
    }
}

/// Звернення до платформи: клієнт і (для CurseForge) ключ API.
pub struct Api<'a> {
    pub client: &'a Client,
    pub source: Source,
    pub key: Option<&'a str>,
}

/// Для якої збірки (і якого типу контенту) шукаємо.
#[derive(Clone, Debug)]
pub struct Target {
    pub minecraft_version: String,
    pub loader: String,
    pub kind: ContentKind,
}

impl Target {
    /// Моди: потрібен завантажувач.
    pub fn new(minecraft_version: &str, loader: &str) -> Result<Self> {
        let loader = loader.trim().to_lowercase();
        if !matches!(loader.as_str(), "fabric" | "forge" | "neoforge" | "quilt") {
            return fail(tr(
                "Моди працюють лише зі збірками на Fabric, Forge, NeoForge або Quilt",
                "Mods only work with Fabric, Forge, NeoForge or Quilt modpacks",
            ));
        }
        Ok(Self {
            minecraft_version: minecraft_version.trim().to_string(),
            loader,
            kind: ContentKind::Mod,
        })
    }

    /// Будь-який тип контенту. Ресурс-паки й шейдери фільтруються лише за версією Minecraft,
    /// готові збірки — без фільтра (версію гри та завантажувач визначає сама збірка).
    pub fn content(kind: ContentKind, minecraft_version: &str, loader: &str) -> Result<Self> {
        match kind {
            ContentKind::Mod => Self::new(minecraft_version, loader),
            ContentKind::Modpack => Ok(Self::modpacks()),
            _ => Ok(Self {
                minecraft_version: minecraft_version.trim().to_string(),
                loader: loader.trim().to_lowercase(),
                kind,
            }),
        }
    }

    pub fn modpacks() -> Self {
        Self {
            minecraft_version: String::new(),
            loader: String::new(),
            kind: ContentKind::Modpack,
        }
    }

    /// Сумісні завантажувачі: Quilt запускає моди Fabric, NeoForge 1.20.1 — моди Forge.
    /// Для ресурс-паків, шейдерів і збірок фільтра за завантажувачем немає.
    pub fn loaders(&self) -> Vec<&'static str> {
        if self.kind != ContentKind::Mod {
            return Vec::new();
        }
        match (self.loader.as_str(), self.minecraft_version.as_str()) {
            ("quilt", _) => vec!["quilt", "fabric"],
            ("neoforge", "1.20.1") => vec!["neoforge", "forge"],
            ("neoforge", _) => vec!["neoforge"],
            ("forge", _) => vec!["forge"],
            _ => vec!["fabric"],
        }
    }
}

#[derive(Serialize, Clone, Debug, Default)]
pub struct ModHit {
    pub id: String,
    pub slug: String,
    pub title: String,
    pub description: String,
    pub icon_url: Option<String>,
    pub downloads: u64,
    pub author: String,
    pub categories: Vec<String>,
    /// Найновіші версії Minecraft проєкту (для карток готових збірок).
    pub game_versions: Vec<String>,
}

#[derive(Serialize, Default)]
pub struct SearchPage {
    pub hits: Vec<ModHit>,
    pub total_hits: u64,
    pub offset: u64,
}

#[derive(Serialize, Clone, Debug)]
pub struct Category {
    pub id: String,
    pub name: String,
}

/// Версія файлу мода для випадного списку.
#[derive(Serialize, Clone, Debug, Default)]
pub struct ModVersion {
    pub id: String,
    pub name: String,
    pub version_number: String,
    /// release | beta | alpha
    pub kind: String,
    pub date: String,
    pub filename: String,
    pub game_versions: Vec<String>,
    pub loaders: Vec<String>,
}

/// Готовий до завантаження файл конкретної версії мода.
pub struct ResolvedFile {
    pub project_id: String,
    pub title: String,
    pub icon_url: Option<String>,
    pub version_id: String,
    pub version_number: String,
    pub url: String,
    pub filename: String,
    pub checksum: Checksum,
    /// Обов'язкові залежності: (project_id, конкретна версія або None).
    pub dependencies: Vec<(String, Option<String>)>,
}

pub enum Resolution {
    File(ResolvedFile),
    /// Мод існує, але сумісної версії немає.
    NoVersion { title: String },
}

/// Ідентифікатори проєктів/версій обох платформ: латиниця, цифри, `-`, `_`.
pub fn check_api_id(value: &str) -> Result<()> {
    let ok = !value.is_empty()
        && value.len() <= 64
        && value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    if ok {
        Ok(())
    } else {
        fail("invalid project id")
    }
}

pub fn pretty_label(raw: &str) -> String {
    let text = raw.replace(['-', '_'], " ");
    let mut chars = text.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

pub async fn search(
    api: &Api<'_>,
    target: &Target,
    query: &str,
    category: Option<&str>,
    offset: u32,
) -> Result<SearchPage> {
    match api.source {
        Source::Modrinth => modrinth::search(api.client, target, query, category, offset).await,
        Source::CurseForge => curseforge::search(api, target, query, category, offset).await,
    }
}

pub async fn categories(api: &Api<'_>, kind: ContentKind) -> Result<Vec<Category>> {
    match api.source {
        Source::Modrinth => modrinth::categories(api.client, kind).await,
        Source::CurseForge => curseforge::categories(api, kind).await,
    }
}

pub async fn versions(api: &Api<'_>, target: &Target, project: &str) -> Result<Vec<ModVersion>> {
    match api.source {
        Source::Modrinth => modrinth::versions(api.client, target, project).await,
        Source::CurseForge => curseforge::versions(api, target, project).await,
    }
}

pub async fn resolve(
    api: &Api<'_>,
    target: &Target,
    project: &str,
    version: Option<&str>,
) -> Result<Resolution> {
    match api.source {
        Source::Modrinth => modrinth::resolve(api.client, target, project, version).await,
        Source::CurseForge => curseforge::resolve(api, target, project, version).await,
    }
}

// ───────────────────────────── встановлені моди ─────────────────────────────

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct ModRecord {
    /// modrinth | curseforge (порожньо в старих записах = modrinth).
    #[serde(default)]
    pub source: String,
    pub project_id: String,
    #[serde(default)]
    pub version_id: String,
    #[serde(default)]
    pub version_number: String,
    pub title: String,
    #[serde(default)]
    pub icon_url: Option<String>,
    /// Ім'я файлу в mods/ (без ".disabled").
    pub filename: String,
    /// Встановлено автоматично як залежність іншого мода.
    #[serde(default)]
    pub dependency: bool,
}

impl ModRecord {
    fn source_id(&self) -> &str {
        if self.source.is_empty() {
            "modrinth"
        } else {
            &self.source
        }
    }
}

#[derive(Serialize, Clone)]
pub struct ModEntry {
    pub source: String,
    pub project_id: String,
    pub title: String,
    pub version_number: String,
    pub icon_url: Option<String>,
    pub filename: String,
    pub enabled: bool,
    pub dependency: bool,
    /// Мод є в mods/, але встановлений не через лаунчер.
    pub manual: bool,
    /// Мод належить збірці проєкту (керується manifest.json — змінювати не можна).
    pub managed: bool,
}

fn records_file(paths: &Paths, id: &str, kind: ContentKind) -> PathBuf {
    paths.instance_meta(id).join(kind.records_name())
}

fn content_dir(paths: &Paths, id: &str, kind: ContentKind) -> PathBuf {
    paths.instance_dir(id).join(kind.folder())
}

fn mods_dir(paths: &Paths, id: &str) -> PathBuf {
    content_dir(paths, id, ContentKind::Mod)
}

async fn load_records(paths: &Paths, id: &str, kind: ContentKind) -> Vec<ModRecord> {
    match tokio::fs::read_to_string(records_file(paths, id, kind)).await {
        // Пошкоджений файл записів не ламає список — моди просто покажуться як «додані вручну».
        Ok(text) => serde_json::from_str(&text).unwrap_or_default(),
        Err(_) => Vec::new(),
    }
}

async fn save_records(paths: &Paths, id: &str, kind: ContentKind, records: &[ModRecord]) -> Result<()> {
    tokio::fs::create_dir_all(paths.instance_meta(id)).await?;
    let target = records_file(paths, id, kind);
    let mut temp = target.clone().into_os_string();
    temp.push(".tmp");
    let temp = PathBuf::from(temp);
    tokio::fs::write(&temp, serde_json::to_string_pretty(records)?).await?;
    tokio::fs::rename(&temp, &target).await?;
    Ok(())
}

/// Зарезервовані імена пристроїв Windows (`CON`, `NUL`, `COM1`…), які не можна використовувати як імена файлів.
pub fn is_reserved_name(name: &str) -> bool {
    let stem = name.split('.').next().unwrap_or("").trim().to_ascii_uppercase();
    matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || ((stem.starts_with("COM") || stem.starts_with("LPT"))
            && stem.len() == 4
            && stem.as_bytes()[3].is_ascii_digit())
}

/// Безпечне ім'я файлу контенту: без шляхів і службових імен, з потрібним розширенням.
fn safe_filename(name: &str, kind: ContentKind) -> Result<String> {
    let name = name.trim();
    // Готові збірки не мають файлів у папці збірки: порожнє розширення пропустило б будь-яке ім'я.
    let bad = kind == ContentKind::Modpack
        || name.is_empty()
        || name.len() > 160
        || name.contains(['/', '\\', ':', '\0', '<', '>', '"', '|', '?', '*'])
        || name.starts_with('.')
        || name.ends_with([' ', '.'])
        || is_reserved_name(name)
        || !name.to_ascii_lowercase().ends_with(kind.extension());
    if bad {
        return fail(t!("Некоректне ім'я файлу: {}", "Invalid file name: {}", name));
    }
    Ok(name.to_string())
}

fn disabled_name(filename: &str) -> String {
    format!("{filename}.disabled")
}

pub async fn count_enabled(paths: &Paths, id: &str) -> usize {
    let mut count = 0;
    if let Ok(mut entries) = tokio::fs::read_dir(mods_dir(paths, id)).await {
        while let Ok(Some(entry)) = entries.next_entry().await {
            if entry.file_name().to_string_lossy().to_lowercase().ends_with(".jar") {
                count += 1;
            }
        }
    }
    count
}

fn title_from_file(filename: &str) -> String {
    let lower = filename.to_ascii_lowercase();
    for ext in [".jar", ".zip"] {
        if lower.ends_with(ext) {
            return filename[..filename.len() - ext.len()].to_string();
        }
    }
    filename.to_string()
}

/// Чи встановлено мод для шейдерів (Iris або Oculus) і чи він увімкнений.
pub async fn has_shader_mod(paths: &Paths, id: &str) -> bool {
    const SHADER_MODS: [&str; 4] = ["iris", "oculus", "YL57xq9U", "GchcoXML"];
    let records = load_records(paths, id, ContentKind::Mod).await;
    let dir = mods_dir(paths, id);
    let by_record = records.iter().any(|r| {
        dir.join(&r.filename).is_file()
            && (SHADER_MODS.contains(&r.project_id.as_str())
                || ["455508", "581495"].contains(&r.project_id.as_str())
                || r.title.to_lowercase().starts_with("iris")
                || r.title.to_lowercase().starts_with("oculus"))
    });
    if by_record {
        return true;
    }
    let Ok(mut entries) = tokio::fs::read_dir(&dir).await else {
        return false;
    };
    while let Ok(Some(entry)) = entries.next_entry().await {
        let name = entry.file_name().to_string_lossy().to_lowercase();
        if name.ends_with(".jar") && (name.starts_with("iris") || name.starts_with("oculus")) {
            return true;
        }
    }
    false
}

/// Вміст власної збірки (моди / ресурс-паки / шейдери): записи лаунчера + файли, додані вручну.
pub async fn list(paths: &Paths, id: &str, kind: ContentKind) -> Vec<ModEntry> {
    if kind == ContentKind::Modpack {
        return Vec::new();
    }
    let mut records = load_records(paths, id, kind).await;
    let dir = content_dir(paths, id, kind);
    // Записи, файлів яких уже немає (гравець видалив вручну), прибираємо.
    let before = records.len();
    records.retain(|r| dir.join(&r.filename).is_file() || dir.join(disabled_name(&r.filename)).is_file());
    if records.len() != before {
        let _ = save_records(paths, id, kind, &records).await;
    }

    let mut out: Vec<ModEntry> = records
        .iter()
        .map(|r| ModEntry {
            source: r.source_id().to_string(),
            project_id: r.project_id.clone(),
            title: r.title.clone(),
            version_number: r.version_number.clone(),
            icon_url: r.icon_url.clone(),
            filename: r.filename.clone(),
            enabled: dir.join(&r.filename).is_file(),
            dependency: r.dependency,
            manual: false,
            managed: false,
        })
        .collect();

    let known: HashSet<String> = records.iter().map(|r| r.filename.to_lowercase()).collect();
    if let Ok(mut entries) = tokio::fs::read_dir(&dir).await {
        while let Ok(Some(entry)) = entries.next_entry().await {
            let name = entry.file_name().to_string_lossy().to_string();
            let (filename, enabled) = match name.strip_suffix(".disabled") {
                Some(base) => (base.to_string(), false),
                None => (name.clone(), true),
            };
            if !filename.to_lowercase().ends_with(kind.extension())
                || known.contains(&filename.to_lowercase())
                || !entry.file_type().await.map(|t| t.is_file()).unwrap_or(false)
            {
                continue;
            }
            out.push(ModEntry {
                source: String::new(),
                project_id: String::new(),
                title: title_from_file(&filename),
                version_number: String::new(),
                icon_url: None,
                filename,
                enabled,
                dependency: false,
                manual: true,
                managed: false,
            });
        }
    }
    out.sort_by(|a, b| a.title.to_lowercase().cmp(&b.title.to_lowercase()));
    out
}

/// Склад збірки проєкту (лише перегляд): моди з manifest.json і сторонні файли в mods/,
/// які дебагер прибере перед наступним запуском.
pub async fn list_managed(paths: &Paths, id: &str, manifest: Option<&Manifest>) -> Vec<ModEntry> {
    let dir = paths.instance_dir(id);
    let mut out: Vec<ModEntry> = Vec::new();
    let mut listed: HashSet<String> = HashSet::new();
    if let Some(manifest) = manifest {
        for file in manifest.files.iter().filter(|f| is_mod_path(&f.path)) {
            let filename = file.path.trim_start_matches("mods/").to_string();
            listed.insert(filename.to_lowercase());
            out.push(ModEntry {
                source: String::new(),
                project_id: String::new(),
                title: title_from_file(filename.rsplit('/').next().unwrap_or(&filename)),
                version_number: String::new(),
                icon_url: None,
                enabled: dir.join(&file.path).is_file(),
                filename,
                dependency: false,
                manual: false,
                managed: true,
            });
        }
    }
    if let Ok(mut entries) = tokio::fs::read_dir(dir.join("mods")).await {
        while let Ok(Some(entry)) = entries.next_entry().await {
            let name = entry.file_name().to_string_lossy().to_string();
            if !name.to_lowercase().ends_with(".jar") || listed.contains(&name.to_lowercase()) {
                continue;
            }
            out.push(ModEntry {
                source: String::new(),
                project_id: String::new(),
                title: title_from_file(&name),
                version_number: String::new(),
                icon_url: None,
                filename: name,
                enabled: true,
                dependency: false,
                manual: true,
                managed: false,
            });
        }
    }
    out.sort_by(|a, b| a.title.to_lowercase().cmp(&b.title.to_lowercase()));
    out
}

// ───────────────────────────── встановлення ─────────────────────────────

#[derive(Serialize, Default)]
pub struct InstallReport {
    /// Назви встановлених модів (головний + залежності).
    pub installed: Vec<String>,
    /// Залежності, для яких не знайшлося сумісної версії.
    pub missing: Vec<String>,
}

/// Встановлює мод / ресурс-пак / шейдер (конкретну версію або найкращу сумісну);
/// для модів — ще й усі обов'язкові залежності.
pub async fn install(
    api: &Api<'_>,
    paths: &Paths,
    id: &str,
    target: &Target,
    project: &str,
    version: Option<String>,
) -> Result<InstallReport> {
    let kind = target.kind;
    if kind == ContentKind::Modpack {
        return fail("modpacks are installed as new instances");
    }
    let dir = content_dir(paths, id, kind);
    tokio::fs::create_dir_all(&dir).await?;
    let mut records = load_records(paths, id, kind).await;
    let mut report = InstallReport::default();
    let source = api.source.id();
    let mut seen: HashSet<String> = records
        .iter()
        .filter(|r| r.source_id() == source)
        .map(|r| r.project_id.clone())
        .collect();

    // (project_id, точна версія або None, чи це залежність)
    let mut queue: Vec<(String, Option<String>, bool)> = vec![(project.to_string(), version, false)];
    let mut first = true;
    while let Some((project_id, version_id, dependency)) = queue.pop() {
        if report.installed.len() >= MAX_INSTALLS {
            break;
        }
        if !first && seen.contains(&project_id) {
            continue;
        }
        let resolution = match resolve(api, target, &project_id, version_id.as_deref()).await {
            Ok(resolution) => resolution,
            Err(error) if first => return Err(error),
            Err(_) => {
                report.missing.push(project_id.clone());
                continue;
            }
        };
        let file = match resolution {
            Resolution::File(file) => file,
            Resolution::NoVersion { title } => {
                if first {
                    return fail(t!(
                        "«{}» не має версії для Minecraft {} ({})",
                        "\"{}\" has no version for Minecraft {} ({})",
                        title,
                        target.minecraft_version,
                        if kind == ContentKind::Mod { target.loader.as_str() } else { kind.folder() }
                    ));
                }
                report.missing.push(title);
                continue;
            }
        };
        first = false;
        if seen.contains(&file.project_id) && dependency {
            continue;
        }
        seen.insert(file.project_id.clone());
        seen.insert(project_id.clone());

        if !file.url.starts_with("https://") {
            return fail("mod url is not https");
        }
        let filename = safe_filename(&file.filename, kind)?;

        // Стару версію цього ж мода прибираємо.
        let old: Vec<ModRecord> = records
            .iter()
            .filter(|r| r.source_id() == source && r.project_id == file.project_id)
            .cloned()
            .collect();
        for record in &old {
            if !record.filename.eq_ignore_ascii_case(&filename) {
                let _ = tokio::fs::remove_file(dir.join(&record.filename)).await;
                let _ = tokio::fs::remove_file(dir.join(disabled_name(&record.filename))).await;
            }
        }
        records.retain(|r| !(r.source_id() == source && r.project_id == file.project_id));

        download_file(api.client, &file.url, &dir.join(&filename), &file.checksum, &noop).await?;
        // Якщо раніше цей файл був вимкнений — лишаємо тільки свіжу увімкнену копію.
        let _ = tokio::fs::remove_file(dir.join(disabled_name(&filename))).await;
        records.push(ModRecord {
            source: source.to_string(),
            project_id: file.project_id.clone(),
            version_id: file.version_id.clone(),
            version_number: file.version_number.clone(),
            title: file.title.clone(),
            icon_url: file.icon_url.clone(),
            filename,
            dependency,
        });
        save_records(paths, id, kind, &records).await?;
        report.installed.push(file.title.clone());

        // Залежності мають сенс лише для модів (шейдер «потребує Iris» — це не файл у shaderpacks/).
        if kind != ContentKind::Mod {
            break;
        }
        for (dep_project, dep_version) in file.dependencies {
            if !seen.contains(&dep_project) {
                queue.push((dep_project, dep_version, true));
            }
        }
    }
    Ok(report)
}

// ───────────────────────────── керування ─────────────────────────────

pub async fn set_enabled(paths: &Paths, id: &str, kind: ContentKind, filename: &str, enabled: bool) -> Result<()> {
    let filename = safe_filename(filename, kind)?;
    let dir = content_dir(paths, id, kind);
    let on = dir.join(&filename);
    let off = dir.join(disabled_name(&filename));
    if enabled && off.is_file() {
        tokio::fs::rename(&off, &on).await?;
    } else if !enabled && on.is_file() {
        tokio::fs::rename(&on, &off).await?;
    }
    Ok(())
}

pub async fn remove(paths: &Paths, id: &str, kind: ContentKind, filename: &str) -> Result<()> {
    let filename = safe_filename(filename, kind)?;
    let dir = content_dir(paths, id, kind);
    let _ = tokio::fs::remove_file(dir.join(&filename)).await;
    let _ = tokio::fs::remove_file(dir.join(disabled_name(&filename))).await;
    let mut records = load_records(paths, id, kind).await;
    records.retain(|r| !r.filename.eq_ignore_ascii_case(&filename));
    save_records(paths, id, kind, &records).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filenames_are_checked_per_kind() {
        assert!(safe_filename("sodium-0.6.jar", ContentKind::Mod).is_ok());
        assert!(safe_filename("Faithful 32x.zip", ContentKind::ResourcePack).is_ok());
        assert!(safe_filename("BSL_v8.zip", ContentKind::Shader).is_ok());
        for bad in ["../x.jar", "a\\b.jar", "C:x.jar", ".hidden.jar", "x.zip", "CON.jar", "nul.jar", "x.jar.", "a|b.jar"] {
            assert!(safe_filename(bad, ContentKind::Mod).is_err(), "{bad}");
        }
        assert!(safe_filename("pack.jar", ContentKind::ResourcePack).is_err());
        // Модпак не має папки у збірці: жодне ім'я (навіть options.txt) не проходить.
        assert!(safe_filename("options.txt", ContentKind::Modpack).is_err());
        assert!(safe_filename("x.jar", ContentKind::Modpack).is_err());
    }

    #[test]
    fn content_kinds_map_to_folders() {
        assert_eq!(ContentKind::parse("resourcepack").unwrap().folder(), "resourcepacks");
        assert_eq!(ContentKind::parse("shader").unwrap().folder(), "shaderpacks");
        assert_eq!(ContentKind::parse("").unwrap(), ContentKind::Mod);
        assert!(ContentKind::parse("worlds").is_err());
        assert_eq!(ContentKind::Modpack.curseforge_class(), 4471);
    }

    #[test]
    fn non_mod_targets_have_no_loader_filter() {
        let target = Target::content(ContentKind::ResourcePack, "1.21.1", "vanilla").unwrap();
        assert!(target.loaders().is_empty());
        assert!(Target::content(ContentKind::Mod, "1.21.1", "vanilla").is_err());
        assert_eq!(Target::content(ContentKind::Mod, "1.20.1", "neoforge").unwrap().loaders(), vec!["neoforge", "forge"]);
    }

    #[test]
    fn reserved_names() {
        assert!(is_reserved_name("COM1.zip"));
        assert!(is_reserved_name("aux"));
        assert!(!is_reserved_name("computer.jar"));
        assert!(!is_reserved_name("console.zip"));
    }
}
