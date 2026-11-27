//! CurseForge Core API (https://api.curseforge.com/v1). Потрібен ключ API
//! (Налаштування → «Джерела модів» або змінна середовища `CURSEFORGE_API_KEY`).

use std::time::Duration;

use serde::de::DeserializeOwned;
use serde::Deserialize;

use crate::error::{fail, Result};
use crate::i18n::tr;
use crate::mods::{
    Api, Category, ContentKind, ModHit, ModVersion, Resolution, ResolvedFile, SearchPage, Target, PAGE,
};
use crate::t;
use crate::util::Checksum;

const API: &str = "https://api.curseforge.com/v1";
const GAME_MINECRAFT: &str = "432";
const TIMEOUT: Duration = Duration::from_secs(20);
/// Скільки id передаємо в одному POST /mods, /mods/files.
const BATCH: usize = 200;
/// API віддає не більше 10 000 результатів пошуку (index + pageSize).
const MAX_INDEX: u32 = 10_000;

fn key<'a>(api: &Api<'a>) -> Result<&'a str> {
    match api.key.map(str::trim).filter(|k| !k.is_empty()) {
        Some(key) => Ok(key),
        None => fail(tr(
            "Для CurseForge потрібен API-ключ. Додай його в Налаштуваннях → «Джерела модів». Modrinth працює без ключа.",
            "CurseForge needs an API key. Add it in Settings → \"Mod sources\". Modrinth works without a key.",
        )),
    }
}

async fn get<T: DeserializeOwned>(api: &Api<'_>, path: &str, query: &[(&str, String)]) -> Result<T> {
    let key = key(api)?;
    let response = api
        .client
        .get(format!("{API}{path}"))
        .header("x-api-key", key)
        .header("Accept", "application/json")
        .query(query)
        .timeout(TIMEOUT)
        .send()
        .await?;
    match response.status().as_u16() {
        401 | 403 => fail(tr(
            "CurseForge відхилив API-ключ (403). Перевір ключ у Налаштуваннях → «Джерела модів».",
            "CurseForge rejected the API key (403). Check the key in Settings → \"Mod sources\".",
        )),
        404 => fail(tr("Проєкт не знайдено на CurseForge", "Project not found on CurseForge")),
        _ => Ok(response.error_for_status()?.json::<T>().await?),
    }
}

async fn post<T: DeserializeOwned>(api: &Api<'_>, path: &str, body: serde_json::Value) -> Result<T> {
    let key = key(api)?;
    let response = api
        .client
        .post(format!("{API}{path}"))
        .header("x-api-key", key)
        .header("Accept", "application/json")
        .json(&body)
        .timeout(TIMEOUT)
        .send()
        .await?;
    match response.status().as_u16() {
        401 | 403 => fail(tr(
            "CurseForge відхилив API-ключ (403). Перевір ключ у Налаштуваннях → «Джерела модів».",
            "CurseForge rejected the API key (403). Check the key in Settings → \"Mod sources\".",
        )),
        _ => Ok(response.error_for_status()?.json::<T>().await?),
    }
}

fn loader_type(loader: &str) -> u32 {
    match loader {
        "forge" => 1,
        "fabric" => 4,
        "quilt" => 5,
        "neoforge" => 6,
        _ => 0,
    }
}

/// Тип завантажувача для пошуку. Моди NeoForge на 1.20.1 здебільшого позначені як Forge,
/// а для Quilt на CurseForge майже все — моди Fabric.
fn search_loader_type(target: &Target) -> u32 {
    match (target.loader.as_str(), target.minecraft_version.as_str()) {
        ("neoforge", "1.20.1") => 1,
        ("quilt", _) => 4,
        (loader, _) => loader_type(loader),
    }
}

fn loader_label(loader: &str) -> &'static str {
    match loader {
        "forge" => "forge",
        "fabric" => "fabric",
        "quilt" => "quilt",
        "neoforge" => "neoforge",
        _ => "",
    }
}

#[derive(Deserialize)]
struct Page<T> {
    data: Vec<T>,
    #[serde(default)]
    pagination: Option<Pagination>,
}

#[derive(Deserialize)]
struct One<T> {
    data: T,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Pagination {
    #[serde(default)]
    index: u64,
    #[serde(default)]
    total_count: u64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CfMod {
    id: u64,
    name: String,
    #[serde(default)]
    slug: String,
    #[serde(default)]
    summary: String,
    #[serde(default)]
    download_count: f64,
    #[serde(default)]
    logo: Option<Logo>,
    #[serde(default)]
    authors: Vec<Author>,
    #[serde(default)]
    categories: Vec<CfCategory>,
    #[serde(default)]
    class_id: Option<u64>,
    #[serde(default)]
    latest_files_indexes: Vec<LatestIndex>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct LatestIndex {
    #[serde(default)]
    game_version: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Logo {
    #[serde(default)]
    thumbnail_url: String,
}

#[derive(Deserialize)]
struct Author {
    name: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CfCategory {
    id: u64,
    name: String,
    #[serde(default)]
    class_id: Option<u64>,
    #[serde(default)]
    is_class: Option<bool>,
}

#[derive(Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct CfFile {
    id: u64,
    mod_id: u64,
    #[serde(default)]
    display_name: String,
    file_name: String,
    #[serde(default)]
    release_type: u32,
    #[serde(default)]
    file_date: String,
    #[serde(default)]
    download_url: Option<String>,
    #[serde(default)]
    hashes: Vec<Hash>,
    #[serde(default)]
    dependencies: Vec<Dependency>,
    #[serde(default)]
    game_versions: Vec<String>,
    #[serde(default)]
    file_length: u64,
}

#[derive(Deserialize, Clone)]
struct Hash {
    value: String,
    algo: u32,
}

#[derive(Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct Dependency {
    mod_id: u64,
    relation_type: u32,
}

const LOADER_TAGS: [&str; 4] = ["forge", "fabric", "quilt", "neoforge"];

impl CfFile {
    fn kind(&self) -> &'static str {
        match self.release_type {
            2 => "beta",
            3 => "alpha",
            _ => "release",
        }
    }

    fn loaders(&self) -> Vec<String> {
        self.game_versions
            .iter()
            .map(|v| v.to_lowercase())
            .filter(|v| LOADER_TAGS.contains(&v.as_str()))
            .collect()
    }

    fn fits(&self, target: &Target) -> bool {
        match target.kind {
            ContentKind::Modpack => return true,
            ContentKind::ResourcePack | ContentKind::Shader => {
                // Частина шейдерпаків на CurseForge взагалі не позначає версій гри.
                let numeric = self
                    .game_versions
                    .iter()
                    .filter(|v| v.chars().next().is_some_and(|c| c.is_ascii_digit()))
                    .count();
                return numeric == 0 || self.game_versions.iter().any(|v| v == &target.minecraft_version);
            }
            ContentKind::Mod => {}
        }
        if !self.game_versions.iter().any(|v| v == &target.minecraft_version) {
            return false;
        }
        let tags = self.loaders();
        if tags.is_empty() {
            // Старі файли без позначки завантажувача — це моди Forge.
            return target.loader == "forge";
        }
        target
            .loaders()
            .iter()
            .any(|loader| tags.iter().any(|tag| tag == loader_label(loader)))
    }
}

pub async fn search(
    api: &Api<'_>,
    target: &Target,
    query: &str,
    category: Option<&str>,
    offset: u32,
) -> Result<SearchPage> {
    let offset = offset.min(MAX_INDEX - PAGE);
    let mut params: Vec<(&str, String)> = vec![
        ("gameId", GAME_MINECRAFT.to_string()),
        ("classId", target.kind.curseforge_class().to_string()),
        ("pageSize", PAGE.to_string()),
        ("index", offset.to_string()),
    ];
    if !target.minecraft_version.is_empty() && target.kind != ContentKind::Shader {
        params.push(("gameVersion", target.minecraft_version.clone()));
    }
    if target.kind == ContentKind::Mod {
        params.push(("modLoaderType", search_loader_type(target).to_string()));
    }
    let query = query.trim();
    if query.is_empty() {
        params.push(("sortField", "2".to_string())); // популярність
        params.push(("sortOrder", "desc".to_string()));
    } else {
        params.push(("searchFilter", query.to_string()));
    }
    if let Some(category) = category.filter(|c| !c.is_empty() && c.chars().all(|ch| ch.is_ascii_digit())) {
        params.push(("categoryId", category.to_string()));
    }
    let page: Page<CfMod> = get(api, "/mods/search", &params).await?;
    let (index, total) = page
        .pagination
        .map(|p| (p.index, p.total_count.min(MAX_INDEX as u64)))
        .unwrap_or((offset as u64, 0));
    Ok(SearchPage {
        offset: index,
        total_hits: total,
        hits: page.data.into_iter().map(hit_of).collect(),
    })
}

fn hit_of(item: CfMod) -> ModHit {
    let mut game_versions: Vec<String> = Vec::new();
    for index in &item.latest_files_indexes {
        let version = index.game_version.trim();
        if !version.is_empty() && !game_versions.iter().any(|v| v == version) && game_versions.len() < 3 {
            game_versions.push(version.to_string());
        }
    }
    ModHit {
        game_versions,
        id: item.id.to_string(),
        slug: item.slug,
        title: item.name,
        description: item.summary,
        icon_url: item
            .logo
            .map(|logo| logo.thumbnail_url)
            .filter(|url| url.starts_with("https://")),
        downloads: item.download_count.max(0.0) as u64,
        author: item.authors.into_iter().next().map(|a| a.name).unwrap_or_default(),
        categories: item.categories.into_iter().map(|c| c.name).collect(),
    }
}

pub async fn categories(api: &Api<'_>, kind: ContentKind) -> Result<Vec<Category>> {
    let class = kind.curseforge_class();
    let page: Page<CfCategory> = get(
        api,
        "/categories",
        &[
            ("gameId", GAME_MINECRAFT.to_string()),
            ("classId", class.to_string()),
        ],
    )
    .await?;
    let mut out: Vec<Category> = page
        .data
        .into_iter()
        .filter(|c| c.class_id == Some(class) && c.is_class != Some(true))
        .map(|c| Category {
            id: c.id.to_string(),
            name: c.name,
        })
        .collect();
    out.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(out)
}

/// Сумісні файли проєкту, від найновішого.
async fn compatible(api: &Api<'_>, target: &Target, project: &str) -> Result<Vec<CfFile>> {
    let mut query = vec![("pageSize", "50".to_string())];
    if !target.minecraft_version.is_empty() && target.kind != ContentKind::Shader {
        query.push(("gameVersion", target.minecraft_version.clone()));
    }
    let page: Page<CfFile> = get(api, &format!("/mods/{project}/files"), &query).await?;
    let mut files: Vec<CfFile> = page.data.into_iter().filter(|f| f.fits(target)).collect();
    files.sort_by(|a, b| b.file_date.cmp(&a.file_date));
    Ok(files)
}

pub async fn versions(api: &Api<'_>, target: &Target, project: &str) -> Result<Vec<ModVersion>> {
    Ok(compatible(api, target, project)
        .await?
        .into_iter()
        .map(|file| ModVersion {
            id: file.id.to_string(),
            name: file.display_name.clone(),
            version_number: if file.display_name.is_empty() {
                file.file_name.clone()
            } else {
                file.display_name.clone()
            },
            kind: file.kind().to_string(),
            date: file.file_date.clone(),
            loaders: file.loaders(),
            filename: file.file_name,
            game_versions: file
                .game_versions
                .into_iter()
                .filter(|v| v.chars().next().is_some_and(|c| c.is_ascii_digit()))
                .collect(),
        })
        .collect())
}

pub async fn resolve(
    api: &Api<'_>,
    target: &Target,
    project: &str,
    version: Option<&str>,
) -> Result<Resolution> {
    let info: One<CfMod> = get(api, &format!("/mods/{project}"), &[]).await?;
    let info = info.data;
    let chosen = match version {
        Some(file_id) => {
            let file: One<CfFile> = get(api, &format!("/mods/{project}/files/{file_id}"), &[]).await?;
            if file.data.mod_id != info.id {
                return fail("file belongs to another mod");
            }
            Some(file.data)
        }
        None => {
            let list = compatible(api, target, project).await?;
            list.iter()
                .find(|f| f.release_type == 1)
                .cloned()
                .or_else(|| list.into_iter().next())
        }
    };
    let Some(file) = chosen else {
        return Ok(Resolution::NoVersion { title: info.name });
    };
    let Some(url) = file.download_url.clone().filter(|u| u.starts_with("https://")) else {
        return fail(t!(
            "Автор «{}» заборонив завантаження через сторонні лаунчери — завантаж файл вручну з сайту CurseForge у папку {}",
            "The author of \"{}\" disabled downloads through third-party launchers — download it manually from the CurseForge website into the {} folder",
            info.name,
            if target.kind == ContentKind::Modpack { "Downloads" } else { target.kind.folder() }
        ));
    };
    let checksum = file
        .hashes
        .iter()
        .find(|h| h.algo == 1 && h.value.len() == 40)
        .map(|h| Checksum::Sha1(h.value.to_lowercase()))
        .unwrap_or(Checksum::None);
    let dependencies = file
        .dependencies
        .iter()
        .filter(|d| d.relation_type == 3)
        .map(|d| (d.mod_id.to_string(), None))
        .collect();
    let icon_url = info
        .logo
        .as_ref()
        .map(|logo| logo.thumbnail_url.clone())
        .filter(|url| url.starts_with("https://"));
    Ok(Resolution::File(ResolvedFile {
        project_id: info.id.to_string(),
        title: info.name,
        icon_url,
        version_id: file.id.to_string(),
        version_number: if file.display_name.is_empty() {
            file.file_name.clone()
        } else {
            file.display_name.clone()
        },
        url,
        filename: file.file_name,
        checksum,
        dependencies,
    }))
}

// ───────────────────────────── готові збірки ─────────────────────────────

/// Файл зі списку CurseForge-модпака (`manifest.json` → `files[]`).
pub struct PackFile {
    pub file_id: u64,
    pub mod_id: u64,
    pub title: String,
    pub filename: String,
    /// `None` — автор заборонив завантаження сторонніми лаунчерами.
    pub url: Option<String>,
    pub checksum: Checksum,
    pub size: u64,
    /// `classId` проєкту: 6 — мод, 12 — ресурс-пак, 6552 — шейдер.
    pub class_id: Option<u64>,
}

/// Інформація про файли модпака пакетними запитами (`POST /mods/files`, `POST /mods`).
pub async fn pack_files(api: &Api<'_>, file_ids: &[u64]) -> Result<Vec<PackFile>> {
    let mut files: Vec<CfFile> = Vec::new();
    for chunk in file_ids.chunks(BATCH) {
        let page: Page<CfFile> = post(api, "/mods/files", serde_json::json!({ "fileIds": chunk })).await?;
        files.extend(page.data);
    }
    let mut mod_ids: Vec<u64> = files.iter().map(|f| f.mod_id).collect();
    mod_ids.sort_unstable();
    mod_ids.dedup();
    let mut mods: std::collections::HashMap<u64, CfMod> = std::collections::HashMap::new();
    for chunk in mod_ids.chunks(BATCH) {
        let page: Page<CfMod> = post(api, "/mods", serde_json::json!({ "modIds": chunk })).await?;
        for item in page.data {
            mods.insert(item.id, item);
        }
    }
    Ok(files
        .into_iter()
        .map(|file| {
            let info = mods.get(&file.mod_id);
            let checksum = file
                .hashes
                .iter()
                .find(|h| h.algo == 1 && h.value.len() == 40)
                .map(|h| Checksum::Sha1(h.value.to_lowercase()))
                .unwrap_or(Checksum::None);
            PackFile {
                file_id: file.id,
                mod_id: file.mod_id,
                title: info
                    .map(|m| m.name.clone())
                    .unwrap_or_else(|| file.display_name.clone()),
                url: file.download_url.clone().filter(|u| u.starts_with("https://")),
                checksum,
                size: file.file_length,
                class_id: info.and_then(|m| m.class_id),
                filename: file.file_name,
            }
        })
        .collect())
}
