use std::collections::{HashMap, HashSet};
use std::time::{Duration, SystemTime};

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
use crate::instance;
use crate::manifest::{self, Manifest};
pub use crate::mirrors::FetchError;
use crate::mirrors;
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
    /// Фони збірки (імена файлів у її папці), якщо список файлів папки відомий із GitHub API:
    /// нумерована серія `1.png..N.png` для слайд-шоу або один запасний фон (`bg.png`, …).
    /// `None` — список невідомий, інтерфейс перевіряє `1.png`, `2.png`, … сам (до першого 404).
    #[serde(default)]
    pub backgrounds: Option<Vec<String>>,
}

/// Одиночні фони збірки в порядку пріоритету — після нумерованої серії `1.png..N.png`.
/// Той самий список використовує інтерфейс (`instanceBackgrounds` в `ui/app.js`).
pub const FALLBACK_BACKGROUNDS: [&str; 7] = [
    "bg.png",
    "bg.jpg",
    "background.png",
    "background.jpg",
    "background.webp",
    "banner.png",
    "banner.jpg",
];

/// Номер кадру слайд-шоу з імені `<N>.png` (`^[1-9]\d*\.png$`): `1.png` → 1, `10.png` → 10.
/// `0.png`, `01.png`, `1.PNG`, `bg.png` та шляхи з `/` — не кадри.
fn frame_number(name: &str) -> Option<&str> {
    let digits = name.strip_suffix(".png")?;
    let mut chars = digits.chars();
    match chars.next() {
        Some('1'..='9') if chars.all(|c| c.is_ascii_digit()) => Some(digits),
        _ => None,
    }
}

/// Нумерована серія фонів, відсортована за числом (`1.png`, `2.png`, …, `10.png`).
/// Порівнюємо довжину, потім цифри — без переповнення навіть для дуже довгих номерів.
pub fn numbered_backgrounds<'a>(names: impl IntoIterator<Item = &'a str>) -> Vec<String> {
    let mut frames: Vec<&str> = names.into_iter().filter(|name| frame_number(name).is_some()).collect();
    frames.sort_by(|a, b| {
        let (a, b) = (frame_number(a).unwrap_or(""), frame_number(b).unwrap_or(""));
        a.len().cmp(&b.len()).then_with(|| a.cmp(b))
    });
    frames.dedup();
    frames.into_iter().map(str::to_string).collect()
}

/// Серія діє, лише якщо в ній є `1.png` — так само, як під час послідовної перевірки
/// `1.png`, `2.png`, … через raw/дзеркала (404 на `1.png` — серії немає).
fn series_or_none(numbered: Vec<String>) -> Option<Vec<String>> {
    (numbered.first().map(String::as_str) == Some("1.png")).then_some(numbered)
}

/// Фони збірки за повним списком файлів її папки: спершу серія `1.png..N.png`
/// (тоді `bg.png` та інші запасні ігноруються), інакше — перший наявний із [`FALLBACK_BACKGROUNDS`].
pub fn pick_backgrounds<'a>(names: impl IntoIterator<Item = &'a str> + Clone) -> Vec<String> {
    if let Some(series) = series_or_none(numbered_backgrounds(names.clone())) {
        return series;
    }
    let present: HashSet<&str> = names.into_iter().collect();
    FALLBACK_BACKGROUNDS
        .iter()
        .find(|name| present.contains(**name))
        .map(|name| vec![name.to_string()])
        .unwrap_or_default()
}

/// Список фонів з індексу/кешу: лише безпечні імена файлів (без шляхів), серія — у числовому порядку.
fn sanitize_backgrounds(list: &[String]) -> Vec<String> {
    if let Some(series) = series_or_none(numbered_backgrounds(list.iter().map(String::as_str))) {
        return series;
    }
    list.iter()
        .find(|name| FALLBACK_BACKGROUNDS.contains(&name.as_str()))
        .map(|name| vec![name.clone()])
        .unwrap_or_default()
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

/// JSON із репозиторію проєкту: оригінальне посилання, а якщо воно недоступне
/// (мережа, таймаут, 403/404/429/451/5xx, сторінка-заглушка) — дзеркала.
async fn get_json<T: DeserializeOwned>(
    client: &Client,
    url: &str,
) -> std::result::Result<T, FetchError> {
    mirrors::get_json(client, url, true).await
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

/// Частина назви репозиторію без префікса `terrapolis-` — у тому регістрі, в якому вона є.
fn strip_prefix_any_case(value: &str) -> &str {
    if value.to_lowercase().starts_with(PROJECT_PREFIX) {
        value.get(PROJECT_PREFIX.len()..).unwrap_or(value)
    } else {
        value
    }
}

/// Назва для інтерфейсу з назви репозиторію. Регістр не змінюємо:
/// `terrapolis-MyTeam` → `MyTeam`, `terrapolis-myteam` → `myteam`.
fn project_title(repo: &str) -> String {
    strip_prefix_any_case(repo)
        .chars()
        .map(|c| if matches!(c, '-' | '_') { ' ' } else { c })
        .collect()
}

/// Як назву генерували до 0.2.5 («Myteam») — щоб розпізнати й виправити збережені назви.
fn legacy_title(repo: &str) -> String {
    let title = project_title(repo);
    let mut chars = title.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

fn trusted_binding(code: &str, name: String, owner: &str, repo: &str, branch: &str) -> ProjectBinding {
    ProjectBinding {
        code: code.to_string(),
        name,
        index_url: raw_url(owner, repo, branch, INDEX_FILE),
    }
}

fn trusted_bindings(code: &str) -> Vec<ProjectBinding> {
    if let Some((owner, repo, branch)) = parse_repo_code(code) {
        if is_trusted_owner(&owner) && is_project_repo(&repo) {
            let name = project_title(&repo);
            return vec![trusted_binding(code, name, &owner, &repo, &branch)];
        }
        return Vec::new();
    }
    // Гравець увів «MyTeam»: показуємо так, як увів, а репозиторій шукаємо в нижньому регістрі.
    let typed = strip_prefix_any_case(code.trim());
    let suffix = typed.to_lowercase();
    if suffix.is_empty() || !is_github_part(&suffix) {
        return Vec::new();
    }
    let repo = format!("{PROJECT_PREFIX}{suffix}");
    TRUSTED_OWNERS
        .iter()
        .map(|owner| trusted_binding(code, typed.to_string(), owner, &repo, "main"))
        .collect()
}

fn name_key(value: &str) -> String {
    value
        .chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

/// Назва проєкту з `projects.json` у корені його репозиторію, наприклад
/// `{"projects": {"MyTeam": {...}}}` або `{"name": "MyTeam"}`.
async fn declared_name(client: &Client, owner: &str, repo: &str, branch: &str) -> Option<String> {
    let url = raw_url(owner, repo, branch, "projects.json");
    let value: serde_json::Value = get_json(client, &url).await.ok()?;
    let clean = |text: &str| -> Option<String> {
        let text = text.trim();
        (!text.is_empty() && text.chars().count() <= 48).then(|| text.to_string())
    };
    if let Some(name) = value.get("name").and_then(|v| v.as_str()).and_then(clean) {
        return Some(name);
    }
    let projects = value.get("projects")?.as_object()?;
    let wanted = name_key(strip_prefix_any_case(repo));
    let named = |key: &String, entry: &serde_json::Value| -> Option<String> {
        entry
            .get("name")
            .and_then(|v| v.as_str())
            .and_then(clean)
            .or_else(|| clean(key))
    };
    if let Some((key, entry)) = projects.iter().find(|(key, _)| name_key(key) == wanted) {
        return named(key, entry);
    }
    if projects.len() == 1 {
        let (key, entry) = projects.iter().next()?;
        return named(key, entry);
    }
    None
}

/// Іконка проєкту — `icon.png` у корені репозиторію (поруч з instances.json).
pub fn project_icon_url(index_url: &str) -> String {
    match github_source(index_url) {
        Some((owner, repo, branch)) => raw_url(&owner, &repo, &branch, "icon.png"),
        None if index_url.starts_with("https://") => format!("{}icon.png", base_of(index_url)),
        None => String::new(),
    }
}

/// Папка збірки в репозиторії (там, де її manifest.json) — для icon.png і background.png.
pub fn instance_asset_base(entry: &InstanceEntry) -> String {
    if entry.manifest.starts_with("https://") {
        base_of(&entry.manifest).to_string()
    } else {
        String::new()
    }
}

/// Назва проєкту для інтерфейсу: з індексу, інакше збережена; стару «Myteam» виправляємо.
pub fn display_name(binding: &ProjectBinding, index: &ProjectIndex) -> String {
    let name = if index.name.trim().is_empty() {
        binding.name.trim().to_string()
    } else {
        index.name.trim().to_string()
    };
    if let Some((_, repo, _)) = github_source(&binding.index_url) {
        if name.is_empty() || name == legacy_title(&repo) {
            let typed = strip_prefix_any_case(binding.code.trim());
            // Код, який увів гравець, точніший за назву репозиторію в нижньому регістрі.
            if !typed.contains('/') && typed.eq_ignore_ascii_case(strip_prefix_any_case(&repo)) {
                return typed.to_string();
            }
            return project_title(&repo);
        }
    }
    if name.is_empty() {
        binding.code.clone()
    } else {
        name
    }
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
    // Та сама назва, але з правильним регістром від адміністратора («myteam» → «MyTeam»).
    let declared = index.name.trim();
    if binding.name.trim().is_empty()
        || (!declared.is_empty() && name_key(&binding.name) == name_key(declared))
    {
        binding.name = declared.to_string();
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
    #[serde(default)]
    pub icon: String,
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

async fn card_of(client: &Client, repo: GithubRepo) -> ProjectCard {
    let branch = repo
        .default_branch
        .as_deref()
        .filter(|branch| is_github_part(branch))
        .unwrap_or("main")
        .to_string();
    let code = if branch == "main" {
        format!("{}/{}", repo.owner.login, repo.name)
    } else {
        format!("{}/{}@{}", repo.owner.login, repo.name, branch)
    };
    let title = declared_name(client, &repo.owner.login, &repo.name, &branch)
        .await
        .unwrap_or_else(|| project_title(&repo.name));
    ProjectCard {
        code,
        icon: raw_url(&repo.owner.login, &repo.name, &branch, "icon.png"),
        title,
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
    let wanted = repos.into_iter().filter(|repo| {
        is_project_repo(&repo.name)
            && is_trusted_owner(&repo.owner.login)
            && !repo.fork
            && !repo.archived
            && !repo.private
            && !repo.disabled
    });
    Ok(join_all(wanted.map(|repo| card_of(client, repo))).await)
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
            backgrounds: None,
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
        if let Some(list) = entry.backgrounds.as_mut() {
            *list = sanitize_backgrounds(list);
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
    let mut found: Vec<(String, String, String)> = Vec::new();
    // Файли безпосередньо в папці кожної збірки (без підпапок) — для вибору фонів.
    let mut folder_files: HashMap<String, Vec<String>> = HashMap::new();
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
        if !file.contains('/') {
            folder_files
                .entry(folder.to_string())
                .or_default()
                .push(file.to_string());
        }
        let id = folder.to_lowercase();
        if file != "manifest.json" || !is_safe_id(&id) || !seen.insert(id.clone()) {
            continue;
        }
        found.push((id, folder.to_string(), node.path.clone()));
    }
    found.sort();

    let entries = join_all(found.into_iter().map(|(id, folder, path)| {
        let client = client.clone();
        let manifest_url = raw_url(owner, repo, branch, &path);
        let files = folder_files.remove(&folder).unwrap_or_default();
        async move {
            let manifest = get_json::<Manifest>(&client, &manifest_url).await.ok();
            let mut entry = instance_from_manifest(id, manifest_url, manifest);
            entry.backgrounds = Some(pick_backgrounds(files.iter().map(String::as_str)));
            entry
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
        let (instances, declared) = futures_util::future::join(
            discover_instances(client, &owner, &repo, &branch),
            declared_name(client, &owner, &repo, &branch),
        )
        .await;
        let instances = instances?;
        if !instances.is_empty() {
            let index = ProjectIndex {
                name: declared.unwrap_or_else(|| binding.name.clone()),
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

/// Індекс зі збірок цього проєкту, що вже встановлені на диску. Потрібен, коли недоступні
/// і всі дзеркала, і кеш індексу: встановлені збірки все одно можна запустити офлайн.
pub async fn local_index(paths: &Paths, binding: &ProjectBinding) -> ProjectIndex {
    let mut instances = Vec::new();
    if let Ok(mut entries) = tokio::fs::read_dir(&paths.instances).await {
        while let Ok(Some(entry)) = entries.next_entry().await {
            let id = entry.file_name().to_string_lossy().to_string();
            if !is_safe_id(&id) {
                continue;
            }
            let state = instance::load_state(paths, &id).await;
            if !state.installed || !state.project.eq_ignore_ascii_case(&binding.code) {
                continue;
            }
            let Some(manifest) = manifest::load_cached(&instance::manifest_cache(paths, &id)).await
            else {
                continue;
            };
            instances.push(InstanceEntry {
                name: if state.name.trim().is_empty() {
                    id.clone()
                } else {
                    state.name.clone()
                },
                id,
                recommended_ram_mb: manifest.recommended_ram_mb,
                minecraft_version: manifest.minecraft_version.clone(),
                loader: manifest.loader.kind.clone(),
                ..InstanceEntry::default()
            });
        }
    }
    instances.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    ProjectIndex {
        name: binding.name.clone(),
        description: String::new(),
        instances,
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

#[cfg(test)]
mod tests {
    use super::*;

    fn binding(code: &str, name: &str) -> ProjectBinding {
        ProjectBinding {
            code: code.to_string(),
            name: name.to_string(),
            index_url: raw_url("hodiw1", "terrapolis-myteam", "main", INDEX_FILE),
        }
    }

    fn index(name: &str) -> ProjectIndex {
        ProjectIndex {
            name: name.to_string(),
            ..ProjectIndex::default()
        }
    }

    #[test]
    fn title_keeps_case() {
        assert_eq!(project_title("terrapolis-myteam"), "myteam");
        assert_eq!(project_title("terrapolis-MyTeam"), "MyTeam");
        assert_eq!(project_title("Terrapolis-my_server"), "my server");
    }

    #[test]
    fn typed_code_keeps_case_but_url_is_lowercase() {
        let found = trusted_bindings("MyTeam");
        assert_eq!(found[0].name, "MyTeam");
        assert!(found[0].index_url.contains("/terrapolis-myteam/"));
        let prefixed = trusted_bindings("Terrapolis-MyTeam");
        assert_eq!(prefixed[0].name, "MyTeam");
        assert!(prefixed[0].index_url.contains("/terrapolis-myteam/"));
    }

    #[test]
    fn display_name_fixes_legacy_capitalisation() {
        // Збережено «Myteam» (стара логіка), адмін назву не вказав.
        assert_eq!(display_name(&binding("hodiw1/terrapolis-myteam", "Myteam"), &index("Myteam")), "myteam");
        assert_eq!(display_name(&binding("MyTeam", "Myteam"), &index("Myteam")), "MyTeam");
        // Адмін вказав назву в projects.json — вона головна.
        assert_eq!(display_name(&binding("hodiw1/terrapolis-myteam", "Myteam"), &index("MyTeam")), "MyTeam");
        assert_eq!(display_name(&binding("x", "Мій сервер"), &index("")), "Мій сервер");
    }

    #[test]
    fn complete_prefers_declared_case() {
        let done = complete(binding("hodiw1/terrapolis-myteam", "myteam"), &index("MyTeam"));
        assert_eq!(done.name, "MyTeam");
        let custom = complete(binding("CODE", "Partner Server"), &index("Something else"));
        assert_eq!(custom.name, "Partner Server");
    }

    #[test]
    fn numbered_backgrounds_sort_by_number() {
        let names = ["10.png", "2.png", "bg.png", "1.png", "3.png", "manifest.json", "11.png", "9.png"];
        assert_eq!(
            numbered_backgrounds(names),
            vec!["1.png", "2.png", "3.png", "9.png", "10.png", "11.png"]
        );
        // Не кадри: нуль на початку, інше розширення чи регістр, префікси, підпапки.
        let junk = ["0.png", "01.png", "1.PNG", "1.jpg", "a1.png", "1a.png", ".png", "1.png.bak", "pack/1.png"];
        assert!(numbered_backgrounds(junk).is_empty());
        // Дуже довгий номер не ламає сортування.
        assert_eq!(
            numbered_backgrounds(["123456789012345678901234567890.png", "5.png"]),
            vec!["5.png", "123456789012345678901234567890.png"]
        );
    }

    #[test]
    fn numbered_series_has_priority_over_bg() {
        let names = ["manifest.json", "icon.png", "bg.png", "banner.jpg", "10.png", "2.png", "1.png"];
        assert_eq!(pick_backgrounds(names), vec!["1.png", "2.png", "10.png"]);
        // Без 1.png..N.png — перший наявний запасний фон у порядку пріоритету.
        assert_eq!(pick_backgrounds(["banner.jpg", "bg.jpg", "icon.png"]), vec!["bg.jpg"]);
        assert_eq!(pick_backgrounds(["icon.png", "background.webp"]), vec!["background.webp"]);
        assert!(pick_backgrounds(["icon.png", "manifest.json"]).is_empty());
        // Без 1.png серії немає (як і при перевірці через HTTP) — береться bg.png.
        assert_eq!(pick_backgrounds(["3.png", "bg.png", "2.png"]), vec!["bg.png"]);
        // Пропуски в нумерації допустимі: усі кадри за числом.
        assert_eq!(pick_backgrounds(["1.png", "5.png", "bg.png"]), vec!["1.png", "5.png"]);
    }

    #[test]
    fn backgrounds_from_index_are_sanitized() {
        let list = |items: &[&str]| items.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(
            sanitize_backgrounds(&list(&["2.png", "../x.png", "1.png", "bg.png"])),
            vec!["1.png", "2.png"]
        );
        assert_eq!(sanitize_backgrounds(&list(&["../bg.png", "bg.png"])), vec!["bg.png"]);
        assert!(sanitize_backgrounds(&list(&["https://evil/x.png"])).is_empty());
    }

    #[test]
    fn asset_urls() {
        assert_eq!(
            project_icon_url(&raw_url("hodiw1", "terrapolis-myteam", "main", INDEX_FILE)),
            "https://raw.githubusercontent.com/hodiw1/terrapolis-myteam/main/icon.png"
        );
        let entry = InstanceEntry {
            manifest: raw_url("hodiw1", "terrapolis-myteam", "main", "instances/fazcraft/manifest.json"),
            ..InstanceEntry::default()
        };
        assert_eq!(
            instance_asset_base(&entry),
            "https://raw.githubusercontent.com/hodiw1/terrapolis-myteam/main/instances/fazcraft/"
        );
    }
}
