use std::collections::{HashMap, HashSet};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use futures_util::future::join_all;
use reqwest::{Client, StatusCode};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::config::ProjectBinding;
use crate::constants::{
    raw_url, registry_url, INDEX_FILE, LEGACY_MANIFEST_FILE, PROJECT_LIST_TTL_SECS, PROJECT_PREFIX,
    TRUSTED_OWNERS,
};
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

fn is_trusted_owner(owner: &str) -> bool {
    TRUSTED_OWNERS
        .iter()
        .any(|trusted| trusted.eq_ignore_ascii_case(owner))
}

fn is_project_repo(name: &str) -> bool {
    let lower = name.to_lowercase();
    is_github_part(name) && lower.starts_with(PROJECT_PREFIX) && lower.len() > PROJECT_PREFIX.len()
}

fn project_title(repo: &str) -> String {
    let suffix = if repo.to_lowercase().starts_with(PROJECT_PREFIX) {
        repo.get(PROJECT_PREFIX.len()..).unwrap_or(repo)
    } else {
        repo
    };
    let spaced: String = suffix
        .chars()
        .map(|c| if matches!(c, '-' | '_') { ' ' } else { c })
        .collect();
    let mut chars = spaced.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

fn trusted_binding(code: &str, owner: &str, repo: &str, branch: &str) -> ProjectBinding {
    ProjectBinding {
        code: code.to_string(),
        name: project_title(repo),
        index_url: raw_url(owner, repo, branch, INDEX_FILE),
    }
}

fn trusted_bindings(code: &str) -> Vec<ProjectBinding> {
    if let Some((owner, repo, branch)) = parse_repo_code(code) {
        if is_trusted_owner(&owner) && is_project_repo(&repo) {
            return vec![trusted_binding(code, &owner, &repo, &branch)];
        }
        return Vec::new();
    }
    let lower = code.to_lowercase();
    let suffix = lower.strip_prefix(PROJECT_PREFIX).unwrap_or(lower.as_str());
    if suffix.is_empty() || !is_github_part(suffix) {
        return Vec::new();
    }
    let repo = format!("{PROJECT_PREFIX}{suffix}");
    TRUSTED_OWNERS
        .iter()
        .map(|owner| trusted_binding(code, owner, &repo, "main"))
        .collect()
}

async fn registry_binding(client: &Client, code: &str) -> Result<Option<ProjectBinding>> {
    match get_json::<Registry>(client, &registry_url()).await {
        Ok(registry) => {
            let Some((key, entry)) = registry
                .projects
                .iter()
                .find(|(key, _)| key.trim().eq_ignore_ascii_case(code))
            else {
                return Ok(None);
            };
            let index_url = entry_index_url(entry).ok_or_else(|| {
                AppError(
                    tr(
                        "Проєкт знайдено, але він налаштований з помилкою. Напиши адміністратору.",
                        "The project exists but is misconfigured. Contact the administrator.",
                    )
                    .to_string(),
                )
            })?;
            Ok(Some(ProjectBinding {
                code: key.trim().to_string(),
                name: entry.name.clone(),
                index_url,
            }))
        }
        Err(FetchError::Offline) => Err(FetchError::Offline.into_app()),
        Err(_) => Ok(None),
    }
}

fn complete(mut binding: ProjectBinding, index: &ProjectIndex) -> ProjectBinding {
    if binding.name.trim().is_empty() {
        binding.name = index.name.clone();
    }
    if binding.name.trim().is_empty() {
        binding.name = binding.code.clone();
    }
    binding
}

pub async fn resolve_code(client: &Client, code: &str) -> Result<(ProjectBinding, ProjectIndex)> {
    let code = code.trim();
    if code.is_empty() {
        return fail(tr("Введи код проєкту", "Enter the project code"));
    }
    if code.len() > 120 {
        return Err(FetchError::NotFound.into_app());
    }

    if let Some(binding) = registry_binding(client, code).await? {
        let index = fetch_index(client, &binding)
            .await
            .map_err(FetchError::into_app)?;
        return Ok((complete(binding, &index), index));
    }

    for binding in trusted_bindings(code) {
        match fetch_index(client, &binding).await {
            Ok(index) => return Ok((complete(binding, &index), index)),
            Err(FetchError::NotFound) => continue,
            Err(error) => return Err(error.into_app()),
        }
    }
    Err(FetchError::NotFound.into_app())
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ProjectCard {
    pub code: String,
    pub title: String,
    pub description: String,
    pub owner: String,
}

#[derive(Deserialize)]
struct GithubOwner {
    login: String,
}

#[derive(Deserialize)]
struct GithubRepo {
    name: String,
    owner: GithubOwner,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    default_branch: Option<String>,
    #[serde(default)]
    fork: bool,
    #[serde(default)]
    archived: bool,
    #[serde(default)]
    private: bool,
    #[serde(default)]
    disabled: bool,
}

#[derive(Deserialize)]
struct TreeNode {
    path: String,
    #[serde(rename = "type", default)]
    kind: String,
}

#[derive(Deserialize, Default)]
struct TreeResponse {
    #[serde(default)]
    tree: Vec<TreeNode>,
}

async fn api_get(client: &Client, url: &str) -> std::result::Result<reqwest::Response, FetchError> {
    client
        .get(url)
        .header("Accept", "application/vnd.github+json")
        .timeout(REQUEST_TIMEOUT)
        .send()
        .await
        .map_err(classify)
}

fn api_status_error(status: StatusCode) -> FetchError {
    if status == StatusCode::FORBIDDEN || status == StatusCode::TOO_MANY_REQUESTS {
        return FetchError::Other(
            tr(
                "GitHub тимчасово обмежив кількість запитів. Спробуй за кілька хвилин.",
                "GitHub is temporarily rate limiting requests. Try again in a few minutes.",
            )
            .to_string(),
        );
    }
    FetchError::Other(format!("GitHub: {status}"))
}

fn card_of(repo: GithubRepo) -> ProjectCard {
    let branch = repo
        .default_branch
        .as_deref()
        .filter(|branch| is_github_part(branch))
        .unwrap_or("main");
    let code = if branch == "main" {
        format!("{}/{}", repo.owner.login, repo.name)
    } else {
        format!("{}/{}@{}", repo.owner.login, repo.name, branch)
    };
    ProjectCard {
        code,
        title: project_title(&repo.name),
        description: repo
            .description
            .as_deref()
            .unwrap_or("")
            .trim()
            .chars()
            .take(140)
            .collect(),
        owner: repo.owner.login,
    }
}

async fn fetch_owner_cards(
    client: &Client,
    owner: &str,
) -> std::result::Result<Vec<ProjectCard>, FetchError> {
    let url = format!("https://api.github.com/users/{owner}/repos?per_page=100&sort=updated");
    let response = api_get(client, &url).await?;
    let status = response.status();
    if status == StatusCode::NOT_FOUND {
        return Ok(Vec::new());
    }
    if !status.is_success() {
        return Err(api_status_error(status));
    }
    let repos: Vec<GithubRepo> = response.json().await.map_err(classify)?;
    Ok(repos
        .into_iter()
        .filter(|repo| {
            is_project_repo(&repo.name)
                && is_trusted_owner(&repo.owner.login)
                && !repo.fork
                && !repo.archived
                && !repo.private
                && !repo.disabled
        })
        .map(card_of)
        .collect())
}

fn list_cache_path(paths: &Paths) -> std::path::PathBuf {
    paths.projects_cache().join("list.json")
}

async fn read_list(path: &std::path::Path) -> Option<Vec<ProjectCard>> {
    let text = tokio::fs::read_to_string(path).await.ok()?;
    serde_json::from_str(&text).ok()
}

async fn read_fresh_list(path: &std::path::Path) -> Option<Vec<ProjectCard>> {
    let modified = tokio::fs::metadata(path).await.ok()?.modified().ok()?;
    let age = SystemTime::now().duration_since(modified).ok()?;
    if age.as_secs() >= PROJECT_LIST_TTL_SECS {
        return None;
    }
    read_list(path).await
}

pub async fn discover_projects(client: &Client, paths: &Paths) -> Result<Vec<ProjectCard>> {
    let path = list_cache_path(paths);
    if let Some(cards) = read_fresh_list(&path).await {
        return Ok(cards);
    }

    let mut cards = Vec::new();
    for owner in TRUSTED_OWNERS {
        match fetch_owner_cards(client, owner).await {
            Ok(found) => cards.extend(found),
            Err(error) => {
                return match read_list(&path).await {
                    Some(saved) => Ok(saved),
                    None => Err(error.into_app()),
                };
            }
        }
    }
    cards.sort_by(|a, b| a.title.to_lowercase().cmp(&b.title.to_lowercase()));

    if let Some(parent) = path.parent() {
        let _ = tokio::fs::create_dir_all(parent).await;
    }
    if let Ok(text) = serde_json::to_string(&cards) {
        let _ = tokio::fs::write(&path, text).await;
    }
    Ok(cards)
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
        return Err(no_instances());
    }
    index.instances = instances;
    Ok(index)
}

fn no_instances() -> FetchError {
    FetchError::Other(
        tr(
            "У проєкті поки немає жодної збірки. Напиши адміністратору.",
            "This project has no modpacks yet. Contact the administrator.",
        )
        .to_string(),
    )
}

pub fn github_source(url: &str) -> Option<(String, String, String)> {
    let rest = url.strip_prefix("https://raw.githubusercontent.com/")?;
    let mut parts = rest.splitn(4, '/');
    let owner = parts.next()?;
    let repo = parts.next()?;
    let branch = parts.next()?;
    if is_github_part(owner) && is_github_part(repo) && is_github_part(branch) {
        Some((owner.to_string(), repo.to_string(), branch.to_string()))
    } else {
        None
    }
}

fn instance_from_manifest(id: String, url: String, manifest: Option<Manifest>) -> InstanceEntry {
    let mut entry = InstanceEntry {
        id: id.clone(),
        name: id,
        manifest: url,
        ..InstanceEntry::default()
    };
    if let Some(manifest) = manifest {
        if !manifest.name.trim().is_empty() {
            entry.name = manifest.name.trim().to_string();
        }
        entry.recommended_ram_mb = manifest.recommended_ram_mb;
        entry.minecraft_version = manifest.minecraft_version.clone();
        entry.loader = manifest.loader.kind.clone();
    }
    entry
}

async fn discover_instances(
    client: &Client,
    owner: &str,
    repo: &str,
    branch: &str,
) -> std::result::Result<Vec<InstanceEntry>, FetchError> {
    let url = format!("https://api.github.com/repos/{owner}/{repo}/git/trees/{branch}?recursive=1");
    let response = api_get(client, &url).await?;
    let status = response.status();
    if status == StatusCode::NOT_FOUND {
        return Err(FetchError::NotFound);
    }
    if status == StatusCode::CONFLICT {
        return Ok(Vec::new());
    }
    if !status.is_success() {
        return Err(api_status_error(status));
    }
    let tree: TreeResponse = response.json().await.map_err(classify)?;

    let mut seen = HashSet::new();
    let mut found: Vec<(String, String)> = Vec::new();
    for node in tree.tree {
        if node.kind != "blob" {
            continue;
        }
        let Some(rest) = node.path.strip_prefix("instances/") else {
            continue;
        };
        let Some((folder, file)) = rest.split_once('/') else {
            continue;
        };
        let id = folder.to_lowercase();
        if file != "manifest.json" || !is_safe_id(&id) || !seen.insert(id.clone()) {
            continue;
        }
        found.push((id, node.path.clone()));
    }
    found.sort();

    let entries = join_all(found.into_iter().map(|(id, path)| {
        let client = client.clone();
        let manifest_url = raw_url(owner, repo, branch, &path);
        async move {
            let manifest = get_json::<Manifest>(&client, &manifest_url).await.ok();
            instance_from_manifest(id, manifest_url, manifest)
        }
    }))
    .await;
    Ok(entries)
}

async fn fetch_without_index(
    client: &Client,
    binding: &ProjectBinding,
    url: &str,
) -> std::result::Result<ProjectIndex, FetchError> {
    if let Some((owner, repo, branch)) = github_source(url) {
        let instances = discover_instances(client, &owner, &repo, &branch).await?;
        if !instances.is_empty() {
            let index = ProjectIndex {
                name: binding.name.clone(),
                description: String::new(),
                instances,
            };
            return normalize(index, url);
        }
    }
    let legacy = format!("{}{}", base_of(url), LEGACY_MANIFEST_FILE);
    match fetch_legacy(client, binding, &legacy).await {
        Err(FetchError::NotFound) => Err(no_instances()),
        other => other,
    }
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
        Err(FetchError::NotFound) => fetch_without_index(client, binding, url).await,
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
