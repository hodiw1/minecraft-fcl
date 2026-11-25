use std::collections::{HashMap, HashSet};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use reqwest::{Client, StatusCode};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::config::ProjectBinding;
use crate::constants::{raw_url, registry_url, INDEX_FILE, LEGACY_MANIFEST_FILE};
use crate::error::{fail, AppError, Result};
use crate::i18n::tr;
use crate::manifest::{self, Manifest};
use crate::paths::Paths;

const REQUEST_TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Deserialize, Default)]
struct Registry {
    #[serde(default)]
    projects: HashMap<String, RegistryEntry>,
}

#[derive(Deserialize, Clone, Default)]
struct RegistryEntry {
    #[serde(default)]
    name: String,
    /// "owner/repo"
    #[serde(default)]
    repo: String,
    #[serde(default)]
    branch: String,
    /// Шлях до індексу всередині репозиторію (за замовчуванням instances.json).
    #[serde(default)]
    index: String,
    /// Альтернатива repo/branch: пряме https-посилання на індекс.
    #[serde(default)]
    index_url: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct InstanceEntry {
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub description: String,
    /// Відносний шлях від instances.json або повне https-посилання на manifest.json збірки.
    #[serde(default)]
    pub manifest: String,
    /// Адреса сервера для статусу "онлайн": host або host:port.
    #[serde(default)]
    pub server: String,
    /// https-посилання на фонову картинку збірки.
    #[serde(default)]
    pub image: String,
    #[serde(default)]
    pub size_mb: Option<u64>,
    #[serde(default)]
    pub recommended_ram_mb: Option<u32>,
    #[serde(default)]
    pub minecraft_version: String,
    #[serde(default)]
    pub loader: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct ProjectIndex {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub instances: Vec<InstanceEntry>,
}

pub enum FetchError {
    Offline,
    NotFound,
    Other(String),
}

impl FetchError {
    pub fn into_app(self) -> AppError {
        match self {
            FetchError::Offline => AppError(
                tr(
                    "Немає з'єднання з інтернетом. Перевір підключення і спробуй ще раз.",
                    "No internet connection. Check your connection and try again.",
                )
                .to_string(),
            ),
            FetchError::NotFound => AppError(
                tr(
                    "Проєкт з таким кодом не знайдено. Перевір, чи правильно введено код.",
                    "No project with this code was found. Check that the code is correct.",
                )
                .to_string(),
            ),
            FetchError::Other(message) => AppError(message),
        }
    }
}

fn classify(error: reqwest::Error) -> FetchError {
    if error.is_connect() || error.is_timeout() || error.is_request() {
        FetchError::Offline
    } else {
        FetchError::Other(error.to_string())
    }
}

fn stamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn bust(url: &str) -> String {
    let separator = if url.contains('?') { '&' } else { '?' };
    format!("{url}{separator}t={}", stamp())
}

async fn get_json<T: DeserializeOwned>(
    client: &Client,
    url: &str,
) -> std::result::Result<T, FetchError> {
    let response = client
        .get(bust(url))
        .timeout(REQUEST_TIMEOUT)
        .send()
        .await
        .map_err(classify)?;
    if response.status() == StatusCode::NOT_FOUND {
        return Err(FetchError::NotFound);
    }
    let response = response
        .error_for_status()
        .map_err(|e| FetchError::Other(e.to_string()))?;
    let text = response.text().await.map_err(classify)?;
    serde_json::from_str(&text)
        .map_err(|e| FetchError::Other(format!("некоректний JSON ({url}): {e}")))
}

pub fn is_safe_id(id: &str) -> bool {
    let mut chars = id.chars();
    match chars.next() {
        Some(first) if first.is_ascii_alphanumeric() => {}
        _ => return false,
    }
    id.len() <= 48
        && id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
}

pub fn slug(code: &str) -> String {
    let slug: String = code
        .trim()
        .to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();
    if slug.is_empty() {
        "project".to_string()
    } else {
        slug
    }
}

fn base_of(url: &str) -> &str {
    match url.rfind('/') {
        Some(position) => &url[..=position],
        None => url,
    }
}

fn resolve_relative(base: &str, value: &str) -> Option<String> {
    let value = value.trim();
    if value.starts_with("https://") {
        return Some(value.to_string());
    }
    let value = value.trim_start_matches("./");
    if value.is_empty() || value.contains("..") || value.contains("://") || value.starts_with('/') {
        return None;
    }
    Some(format!("{base}{value}"))
}

fn is_github_part(value: &str) -> bool {
    !value.is_empty()
        && value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
}

/// Розширений формат коду для адміністраторів: `owner/repo` або `owner/repo@branch`.
fn parse_repo_code(code: &str) -> Option<(String, String, String)> {
    let (repo_part, branch) = match code.split_once('@') {
        Some((repo, branch)) => (repo, branch),
        None => (code, "main"),
    };
    let (owner, repo) = repo_part.split_once('/')?;
    if is_github_part(owner) && is_github_part(repo) && is_github_part(branch) {
        Some((owner.to_string(), repo.to_string(), branch.to_string()))
    } else {
        None
    }
}

fn entry_index_url(entry: &RegistryEntry) -> Option<String> {
    if entry.index_url.starts_with("https://") {
        return Some(entry.index_url.clone());
    }
    let (owner, repo) = entry.repo.split_once('/')?;
    let branch = if entry.branch.trim().is_empty() {
        "main"
    } else {
        entry.branch.trim()
    };
    let index = if entry.index.trim().is_empty() {
        INDEX_FILE
    } else {
        entry.index.trim().trim_start_matches('/')
    };
    if !is_github_part(owner)
        || !is_github_part(repo)
        || !is_github_part(branch)
        || index.contains("..")
    {
        return None;
    }
    Some(raw_url(owner, repo, branch, index))
}

/// Перетворює код, введений гравцем, на прив'язку до джерела збірок.
/// Не зберігає нічого — лише перевіряє, що код існує і індекс читається.
pub async fn resolve_code(client: &Client, code: &str) -> Result<(ProjectBinding, ProjectIndex)> {
    let code = code.trim();
    if code.is_empty() {
        return fail(tr("Введи код проєкту", "Enter the project code"));
    }
    if code.len() > 120 {
        return Err(FetchError::NotFound.into_app());
    }

    let mut binding: Option<ProjectBinding> = None;

    if code.starts_with("https://") {
        binding = Some(ProjectBinding {
            code: code.to_string(),
            name: String::new(),
            index_url: code.to_string(),
        });
    }

    if binding.is_none() {
        match get_json::<Registry>(client, &registry_url()).await {
            Ok(registry) => {
                let found = registry
                    .projects
                    .iter()
                    .find(|(key, _)| key.trim().eq_ignore_ascii_case(code));
                if let Some((key, entry)) = found {
                    let index_url = entry_index_url(entry).ok_or_else(|| {
                        AppError(
                            tr(
                                "Проєкт знайдено, але він налаштований з помилкою. Напиши адміністратору.",
                                "The project exists but is misconfigured. Contact the administrator.",
                            )
                            .to_string(),
                        )
                    })?;
                    binding = Some(ProjectBinding {
                        code: key.trim().to_string(),
                        name: entry.name.clone(),
                        index_url,
                    });
                }
            }
            Err(FetchError::Offline) => return Err(FetchError::Offline.into_app()),
            // Реєстру може не бути (наприклад, у форках) — тоді працюють лише прямі коди.
            Err(_) => {}
        }
    }

    if binding.is_none() {
        if let Some((owner, repo, branch)) = parse_repo_code(code) {
            binding = Some(ProjectBinding {
                code: code.to_string(),
                name: String::new(),
                index_url: raw_url(&owner, &repo, &branch, INDEX_FILE),
            });
        }
    }

    let mut binding = binding.ok_or_else(|| FetchError::NotFound.into_app())?;
    let index = fetch_index(client, &binding)
        .await
        .map_err(FetchError::into_app)?;
    if binding.name.trim().is_empty() {
        binding.name = index.name.clone();
    }
    if binding.name.trim().is_empty() {
        binding.name = binding.code.clone();
    }
    Ok((binding, index))
}

fn legacy_index(binding: &ProjectBinding, url: &str, manifest: &Manifest) -> ProjectIndex {
    let name = if manifest.name.trim().is_empty() {
        binding.name.clone()
    } else {
        manifest.name.clone()
    };
    ProjectIndex {
        name: binding.name.clone(),
        description: String::new(),
        instances: vec![InstanceEntry {
            id: "main".to_string(),
            name,
            description: String::new(),
            manifest: url.to_string(),
            server: String::new(),
            image: String::new(),
            size_mb: None,
            recommended_ram_mb: manifest.recommended_ram_mb,
            minecraft_version: manifest.minecraft_version.clone(),
            loader: manifest.loader.kind.clone(),
        }],
    }
}

async fn fetch_legacy(
    client: &Client,
    binding: &ProjectBinding,
    url: &str,
) -> std::result::Result<ProjectIndex, FetchError> {
    let manifest: Manifest = get_json(client, url).await?;
    Ok(legacy_index(binding, url, &manifest))
}

fn normalize(
    mut index: ProjectIndex,
    index_url: &str,
) -> std::result::Result<ProjectIndex, FetchError> {
    let base = base_of(index_url).to_string();
    let mut seen = HashSet::new();
    let mut instances = Vec::new();
    for mut entry in index.instances.drain(..) {
        entry.id = entry.id.trim().to_lowercase();
        if !is_safe_id(&entry.id) || !seen.insert(entry.id.clone()) {
            continue;
        }
        let manifest_ref = if entry.manifest.trim().is_empty() {
            format!("{}/manifest.json", entry.id)
        } else {
            entry.manifest.clone()
        };
        let Some(url) = resolve_relative(&base, &manifest_ref) else {
            continue;
        };
        entry.manifest = url;
        if entry.name.trim().is_empty() {
            entry.name = entry.id.clone();
        }
        if !entry.image.starts_with("https://") {
            entry.image.clear();
        }
        entry.server = entry
            .server
            .trim()
            .chars()
            .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | ':' | '_'))
            .collect();
        if !manifest::is_safe_token(entry.minecraft_version.trim()) {
            entry.minecraft_version.clear();
        }
        instances.push(entry);
    }
    if instances.is_empty() {
        return Err(FetchError::Other(
            tr(
                "У проєкті поки немає жодної збірки. Напиши адміністратору.",
                "This project has no modpacks yet. Contact the administrator.",
            )
            .to_string(),
        ));
    }
    index.instances = instances;
    Ok(index)
}

pub async fn fetch_index(
    client: &Client,
    binding: &ProjectBinding,
) -> std::result::Result<ProjectIndex, FetchError> {
    let url = binding.index_url.as_str();
    if url.ends_with(LEGACY_MANIFEST_FILE) {
        return fetch_legacy(client, binding, url).await;
    }
    match get_json::<ProjectIndex>(client, url).await {
        Ok(index) => normalize(index, url),
        Err(FetchError::NotFound) => {
            let legacy = format!("{}{}", base_of(url), LEGACY_MANIFEST_FILE);
            fetch_legacy(client, binding, &legacy).await
        }
        Err(other) => Err(other),
    }
}

fn cache_path(paths: &Paths, binding: &ProjectBinding) -> std::path::PathBuf {
    paths
        .projects_cache()
        .join(format!("{}.json", slug(&binding.code)))
}

pub async fn save_cache(
    paths: &Paths,
    binding: &ProjectBinding,
    index: &ProjectIndex,
) -> Result<()> {
    let path = cache_path(paths, binding);
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    tokio::fs::write(&path, serde_json::to_string_pretty(index)?).await?;
    Ok(())
}

pub async fn load_cache(paths: &Paths, binding: &ProjectBinding) -> Option<ProjectIndex> {
    let text = tokio::fs::read_to_string(cache_path(paths, binding))
        .await
        .ok()?;
    serde_json::from_str(&text).ok()
}

/// Індекс проєкту з мережі, а якщо мережі немає — з локального кешу.
/// Повертає (індекс, offline).
pub async fn load_index(
    client: &Client,
    paths: &Paths,
    binding: &ProjectBinding,
) -> Result<(ProjectIndex, bool)> {
    match fetch_index(client, binding).await {
        Ok(index) => {
            let _ = save_cache(paths, binding, &index).await;
            Ok((index, false))
        }
        Err(error) => match load_cache(paths, binding).await {
            Some(index) => Ok((index, true)),
            None => Err(error.into_app()),
        },
    }
}

/// Кеш останнього завантаженого manifest.json збірки (для розміру/версії ще до встановлення).
pub fn remote_manifest_cache(
    paths: &Paths,
    binding: &ProjectBinding,
    id: &str,
) -> std::path::PathBuf {
    paths
        .projects_cache()
        .join("manifests")
        .join(format!("{}--{}.json", slug(&binding.code), id))
}

pub fn find<'a>(index: &'a ProjectIndex, id: &str) -> Option<&'a InstanceEntry> {
    index.instances.iter().find(|entry| entry.id == id)
}
