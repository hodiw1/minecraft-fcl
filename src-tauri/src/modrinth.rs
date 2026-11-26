//! Modrinth API v2 (https://api.modrinth.com/v2) — працює без ключа.

use std::time::Duration;

use reqwest::Client;
use serde::de::DeserializeOwned;
use serde::Deserialize;
use serde_json::json;

use crate::error::{fail, Context, Result};
use crate::i18n::tr;
use crate::mods::{
    pretty_label, Category, ModHit, ModVersion, Resolution, ResolvedFile, SearchPage, Target, PAGE,
};
use crate::util::Checksum;

const API: &str = "https://api.modrinth.com/v2";
const TIMEOUT: Duration = Duration::from_secs(20);

async fn get<T: DeserializeOwned>(client: &Client, path: &str, query: &[(&str, String)]) -> Result<T> {
    let response = client
        .get(format!("{API}{path}"))
        .query(query)
        .timeout(TIMEOUT)
        .send()
        .await?;
    if response.status().as_u16() == 404 {
        return fail(tr("Мод не знайдено на Modrinth", "Mod not found on Modrinth"));
    }
    Ok(response.error_for_status()?.json::<T>().await?)
}

#[derive(Deserialize)]
struct Hit {
    project_id: String,
    #[serde(default)]
    slug: String,
    title: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    icon_url: Option<String>,
    #[serde(default)]
    downloads: u64,
    #[serde(default)]
    author: String,
    #[serde(default)]
    display_categories: Vec<String>,
}

#[derive(Deserialize)]
struct SearchResponse {
    hits: Vec<Hit>,
    #[serde(default)]
    total_hits: u64,
    #[serde(default)]
    offset: u64,
}

pub async fn search(
    client: &Client,
    target: &Target,
    query: &str,
    category: Option<&str>,
    offset: u32,
) -> Result<SearchPage> {
    let loaders: Vec<String> = target
        .loaders()
        .iter()
        .map(|loader| format!("categories:{loader}"))
        .collect();
    let mut facets = vec![
        json!(["project_type:mod"]),
        json!([format!("versions:{}", target.minecraft_version)]),
        json!(loaders),
    ];
    if let Some(category) = category.filter(|c| !c.is_empty()) {
        facets.push(json!([format!("categories:{category}")]));
    }
    let index = if query.trim().is_empty() { "downloads" } else { "relevance" };
    let page: SearchResponse = get(
        client,
        "/search",
        &[
            ("query", query.trim().to_string()),
            ("facets", json!(facets).to_string()),
            ("index", index.to_string()),
            ("limit", PAGE.to_string()),
            ("offset", offset.to_string()),
        ],
    )
    .await?;
    Ok(SearchPage {
        total_hits: page.total_hits,
        offset: page.offset,
        hits: page
            .hits
            .into_iter()
            .map(|hit| ModHit {
                id: hit.project_id,
                slug: hit.slug,
                title: hit.title,
                description: hit.description,
                icon_url: hit.icon_url.filter(|url| url.starts_with("https://")),
                downloads: hit.downloads,
                author: hit.author,
                categories: hit.display_categories,
            })
            .collect(),
    })
}

#[derive(Deserialize)]
struct Tag {
    name: String,
    #[serde(default)]
    project_type: String,
    #[serde(default)]
    header: String,
}

pub async fn categories(client: &Client) -> Result<Vec<Category>> {
    let tags: Vec<Tag> = get(client, "/tag/category", &[]).await?;
    let mut out: Vec<Category> = tags
        .into_iter()
        .filter(|tag| tag.project_type == "mod" && tag.header == "categories")
        .map(|tag| Category {
            name: pretty_label(&tag.name),
            id: tag.name,
        })
        .collect();
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out.dedup_by(|a, b| a.id == b.id);
    Ok(out)
}

#[derive(Deserialize, Clone)]
struct Version {
    id: String,
    project_id: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    version_number: String,
    #[serde(default)]
    version_type: String,
    #[serde(default)]
    date_published: String,
    #[serde(default)]
    game_versions: Vec<String>,
    #[serde(default)]
    loaders: Vec<String>,
    #[serde(default)]
    files: Vec<VersionFile>,
    #[serde(default)]
    dependencies: Vec<Dependency>,
}

#[derive(Deserialize, Clone)]
struct VersionFile {
    url: String,
    filename: String,
    #[serde(default)]
    primary: bool,
    #[serde(default)]
    hashes: FileHashes,
}

#[derive(Deserialize, Clone, Default)]
struct FileHashes {
    #[serde(default)]
    sha1: Option<String>,
    #[serde(default)]
    sha512: Option<String>,
}

#[derive(Deserialize, Clone)]
struct Dependency {
    #[serde(default)]
    project_id: Option<String>,
    #[serde(default)]
    version_id: Option<String>,
    #[serde(default)]
    dependency_type: String,
}

#[derive(Deserialize)]
struct Project {
    id: String,
    title: String,
    #[serde(default)]
    icon_url: Option<String>,
}

impl Version {
    fn primary_file(&self) -> Option<&VersionFile> {
        self.files
            .iter()
            .find(|f| f.primary)
            .or_else(|| self.files.first())
    }
}

/// Сумісні версії мода, від найновішої.
async fn compatible(client: &Client, target: &Target, project: &str) -> Result<Vec<Version>> {
    let loaders = json!(target.loaders()).to_string();
    let games = json!([target.minecraft_version]).to_string();
    let mut list: Vec<Version> = get(
        client,
        &format!("/project/{project}/version"),
        &[("loaders", loaders), ("game_versions", games)],
    )
    .await?;
    list.sort_by(|a, b| b.date_published.cmp(&a.date_published));
    Ok(list)
}

pub async fn versions(client: &Client, target: &Target, project: &str) -> Result<Vec<ModVersion>> {
    Ok(compatible(client, target, project)
        .await?
        .into_iter()
        .take(60)
        .map(|v| ModVersion {
            filename: v.primary_file().map(|f| f.filename.clone()).unwrap_or_default(),
            id: v.id,
            name: v.name,
            version_number: v.version_number,
            kind: if v.version_type.is_empty() {
                "release".to_string()
            } else {
                v.version_type
            },
            date: v.date_published,
            game_versions: v.game_versions,
            loaders: v.loaders,
        })
        .collect())
}

pub async fn resolve(
    client: &Client,
    target: &Target,
    project: &str,
    version: Option<&str>,
) -> Result<Resolution> {
    let info: Project = get(client, &format!("/project/{project}"), &[]).await?;
    let chosen = match version {
        Some(version_id) => {
            let version: Version = get(client, &format!("/version/{version_id}"), &[]).await?;
            if version.project_id != info.id {
                return fail("version belongs to another project");
            }
            Some(version)
        }
        None => {
            let list = compatible(client, target, &info.id).await?;
            // Релізи мають перевагу над бета/альфа.
            list.iter()
                .find(|v| v.version_type == "release")
                .cloned()
                .or_else(|| list.into_iter().next())
        }
    };
    let Some(version) = chosen else {
        return Ok(Resolution::NoVersion { title: info.title });
    };
    let file = version
        .primary_file()
        .cloned()
        .ctx(tr("У версії мода немає файлу", "The mod version has no file"))?;
    let checksum = match (&file.hashes.sha512, &file.hashes.sha1) {
        (Some(hash), _) if hash.len() == 128 => Checksum::Sha512(hash.clone()),
        (_, Some(hash)) if hash.len() == 40 => Checksum::Sha1(hash.clone()),
        _ => Checksum::None,
    };

    let mut dependencies = Vec::new();
    for dep in version
        .dependencies
        .iter()
        .filter(|d| d.dependency_type == "required")
    {
        let project_id = match (&dep.project_id, &dep.version_id) {
            (Some(project_id), _) => project_id.clone(),
            (None, Some(version_id)) => {
                match get::<Version>(client, &format!("/version/{version_id}"), &[]).await {
                    Ok(version) => version.project_id,
                    Err(_) => continue,
                }
            }
            _ => continue,
        };
        dependencies.push((project_id, dep.version_id.clone()));
    }

    Ok(Resolution::File(ResolvedFile {
        project_id: info.id,
        title: info.title,
        icon_url: info.icon_url.filter(|url| url.starts_with("https://")),
        version_id: version.id,
        version_number: version.version_number,
        url: file.url,
        filename: file.filename,
        checksum,
        dependencies,
    }))
}
