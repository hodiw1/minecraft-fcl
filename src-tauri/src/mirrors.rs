//! Мультидзеркальне завантаження (Multi-Mirror Fallback).
//!
//! Будь-яке посилання на файл у GitHub-репозиторії (`raw.githubusercontent.com`,
//! `cdn.jsdelivr.net/gh`, `github.com/<o>/<r>/raw/...`) розгортається у список кандидатів
//! за шаблонами з [`CONTENT_MIRRORS`] і віддаленого конфігу. Якщо кандидат повертає мережеву
//! помилку, таймаут, 403/404/429/451/5xx або сторінку-заглушку замість JSON, запит прозоро
//! повторюється через наступне дзеркало. Хости, що не відповідали, на 5 хвилин переносяться
//! в кінець черги, щоб не чекати таймаут на кожному файлі.

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use reqwest::{Client, StatusCode};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::constants::{
    raw_url, CONTENT_MIRRORS, GITHUB_BRANCH, GITHUB_OWNER, GITHUB_REPO, MIRRORS_CONFIG_URL,
    UPDATE_MIRRORS,
};
use crate::error::AppError;

/// Кеш останнього успішно завантаженого віддаленого конфігу (у папці даних лаунчера).
pub const CACHE_FILE: &str = "mirrors.json";
/// Власні дзеркала гравця/адміністратора — лаунчер цей файл ніколи не перезаписує.
pub const CUSTOM_FILE: &str = "mirrors.custom.json";

pub const JSON_TIMEOUT: Duration = Duration::from_secs(20);
const CONFIG_TIMEOUT: Duration = Duration::from_secs(8);
const DOWN_FOR: Duration = Duration::from_secs(300);
const MAX_TEMPLATES: usize = 16;

pub enum FetchError {
    /// Жодне джерело не відповіло (немає мережі або все заблоковано на рівні з'єднання).
    Offline,
    /// Хоча б одне джерело відповіло, що файлу немає.
    NotFound,
    Other(String),
}

impl FetchError {
    /// Технічний текст — інтерфейс перекладає його людською мовою за ключовими словами.
    pub fn into_plain(self, url: &str) -> AppError {
        match self {
            FetchError::Offline => AppError(format!(
                "error sending request (all mirrors unreachable): {url}"
            )),
            FetchError::NotFound => AppError(format!("HTTP 404 Not Found: {url}")),
            FetchError::Other(message) => AppError(message),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct MirrorsConfig {
    #[serde(default)]
    pub content: Vec<String>,
    #[serde(default, alias = "updates")]
    pub update: Vec<String>,
}

#[derive(Clone, Debug)]
struct MirrorSet {
    content: Vec<String>,
    update: Vec<String>,
    source: &'static str,
}

static ACTIVE: RwLock<Option<Arc<MirrorSet>>> = RwLock::new(None);
static DOWN: Mutex<Option<HashMap<String, Instant>>> = Mutex::new(None);

// ───────────────────────────── шаблони ─────────────────────────────

fn clean_template(raw: &str) -> Option<String> {
    let template = raw.trim().trim_end_matches('/');
    let ok = template.len() <= 300
        && template.starts_with("https://")
        && template.len() > "https://".len() + 3
        && !template
            .chars()
            .any(|c| c.is_whitespace() || matches!(c, '"' | '\\' | '<' | '>'));
    ok.then(|| template.to_string())
}

fn is_content_template(template: &str) -> bool {
    template.contains("{url}") || (template.contains("{owner}") && template.contains("{repo}"))
}

fn builtin() -> MirrorSet {
    build(None, None, "builtin")
}

/// Порядок: власні дзеркала → віддалений конфіг → вбудовані. Дублікати прибираються.
fn build(custom: Option<MirrorsConfig>, remote: Option<MirrorsConfig>, source: &'static str) -> MirrorSet {
    let mut content: Vec<String> = Vec::new();
    let mut update: Vec<String> = Vec::new();
    let builtin = MirrorsConfig {
        content: CONTENT_MIRRORS.iter().map(|s| s.to_string()).collect(),
        update: UPDATE_MIRRORS.iter().map(|s| s.to_string()).collect(),
    };
    for config in [custom, remote, Some(builtin)].into_iter().flatten() {
        for template in config.content.iter().filter_map(|t| clean_template(t)) {
            if is_content_template(&template) && !content.contains(&template) {
                content.push(template);
            }
        }
        for template in config.update.iter().filter_map(|t| clean_template(t)) {
            if !template.contains("{url}") && !update.contains(&template) {
                update.push(template);
            }
        }
    }
    content.truncate(MAX_TEMPLATES);
    update.truncate(MAX_TEMPLATES);
    MirrorSet {
        content,
        update,
        source,
    }
}

fn active() -> Arc<MirrorSet> {
    if let Ok(guard) = ACTIVE.read() {
        if let Some(set) = guard.as_ref() {
            return set.clone();
        }
    }
    Arc::new(builtin())
}

fn apply(set: MirrorSet) {
    if let Ok(mut guard) = ACTIVE.write() {
        *guard = Some(Arc::new(set));
    }
}

fn expand(template: &str, owner: &str, repo: &str, branch: &str) -> String {
    template
        .replace("{owner}", owner)
        .replace("{repo}", repo)
        .replace("{branch}", branch)
        .trim_end_matches('/')
        .to_string()
}

// ───────────────────────────── розбір посилань ─────────────────────────────

#[derive(Clone, Debug, PartialEq)]
pub struct RepoFile {
    pub owner: String,
    pub repo: String,
    pub branch: String,
    pub path: String,
}

fn is_part(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 100
        && value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
}

fn split_query(url: &str) -> (&str, &str) {
    match url.find('?') {
        Some(index) => (&url[..index], &url[index..]),
        None => (url, ""),
    }
}

const JSDELIVR_HOSTS: [&str; 4] = [
    "https://cdn.jsdelivr.net/gh/",
    "https://fastly.jsdelivr.net/gh/",
    "https://gcore.jsdelivr.net/gh/",
    "https://testingcf.jsdelivr.net/gh/",
];

/// Посилання на файл у GitHub-репозиторії → (owner, repo, branch, path).
pub fn repo_file(url: &str) -> Option<RepoFile> {
    let (base, _) = split_query(url);
    let (owner, repo, branch, path) = if let Some(rest) =
        base.strip_prefix("https://raw.githubusercontent.com/")
    {
        let mut parts = rest.splitn(4, '/');
        (parts.next()?, parts.next()?, parts.next()?, parts.next()?)
    } else if let Some(rest) = JSDELIVR_HOSTS.iter().find_map(|host| base.strip_prefix(host)) {
        let mut parts = rest.splitn(3, '/');
        let owner = parts.next()?;
        let (repo, branch) = parts.next()?.split_once('@')?;
        (owner, repo, branch, parts.next()?)
    } else if let Some(rest) = base.strip_prefix("https://github.com/") {
        let mut parts = rest.splitn(5, '/');
        let owner = parts.next()?;
        let repo = parts.next()?;
        if parts.next()? != "raw" {
            return None;
        }
        (owner, repo, parts.next()?, parts.next()?)
    } else {
        return None;
    };
    let path_ok = !path.is_empty() && !path.contains("..") && !path.starts_with('/');
    (is_part(owner) && is_part(repo) && is_part(branch) && path_ok).then(|| RepoFile {
        owner: owner.to_string(),
        repo: repo.to_string(),
        branch: branch.to_string(),
        path: path.to_string(),
    })
}

/// Усі адреси, за якими можна отримати той самий файл: оригінал першим, далі дзеркала.
pub fn content_candidates(url: &str) -> Vec<String> {
    let set = active();
    let (_, query) = split_query(url);
    let file = repo_file(url);
    let canonical = match &file {
        Some(f) => format!("{}{query}", raw_url(&f.owner, &f.repo, &f.branch, &f.path)),
        None => url.to_string(),
    };
    let mut out = vec![url.to_string()];
    for template in &set.content {
        let candidate = if template.contains("{url}") {
            template.replace("{url}", &canonical)
        } else if let Some(f) = &file {
            format!(
                "{}/{}{query}",
                expand(template, &f.owner, &f.repo, &f.branch),
                f.path
            )
        } else {
            continue;
        };
        if !out.contains(&candidate) {
            out.push(candidate);
        }
    }
    healthy_first(out)
}

/// Джерела інформації про оновлення лаунчера (з урахуванням дзеркал контенту), без дублікатів.
pub fn update_candidates() -> Vec<String> {
    let set = active();
    let mut out: Vec<String> = Vec::new();
    for template in &set.update {
        let url = expand(template, GITHUB_OWNER, GITHUB_REPO, GITHUB_BRANCH);
        for candidate in content_candidates(&url) {
            if !out.contains(&candidate) {
                out.push(candidate);
            }
        }
    }
    out
}

// ───────────────────────────── стан хостів ─────────────────────────────

fn host_of(url: &str) -> String {
    url.strip_prefix("https://")
        .unwrap_or(url)
        .split(['/', '?'])
        .next()
        .unwrap_or("")
        .to_lowercase()
}

pub fn mark_down(url: &str) {
    if let Ok(mut guard) = DOWN.lock() {
        guard
            .get_or_insert_with(HashMap::new)
            .insert(host_of(url), Instant::now());
    }
}

pub fn mark_up(url: &str) {
    if let Ok(mut guard) = DOWN.lock() {
        if let Some(map) = guard.as_mut() {
            map.remove(&host_of(url));
        }
    }
}

fn down_hosts() -> Vec<String> {
    let Ok(mut guard) = DOWN.lock() else {
        return Vec::new();
    };
    let map = guard.get_or_insert_with(HashMap::new);
    map.retain(|_, since| since.elapsed() < DOWN_FOR);
    let mut hosts: Vec<String> = map.keys().cloned().collect();
    hosts.sort();
    hosts
}

fn healthy_first(list: Vec<String>) -> Vec<String> {
    let down = down_hosts();
    if down.is_empty() || list.len() < 2 {
        return list;
    }
    let (bad, good): (Vec<String>, Vec<String>) = list
        .into_iter()
        .partition(|url| down.contains(&host_of(url)));
    good.into_iter().chain(bad).collect()
}

// ───────────────────────────── запити ─────────────────────────────

/// Статуси, після яких варто пробувати наступне дзеркало.
pub fn is_retryable(status: StatusCode) -> bool {
    matches!(status.as_u16(), 403 | 404 | 408 | 410 | 429 | 451) || status.is_server_error()
}

/// Статуси, які означають проблему саме з хостом (блокування, ліміт, збій).
pub fn is_host_failure(status: StatusCode) -> bool {
    matches!(status.as_u16(), 403 | 408 | 429 | 451) || status.is_server_error()
}

pub fn is_network(error: &reqwest::Error) -> bool {
    error.is_connect() || error.is_timeout() || error.is_request() || error.is_body()
}

fn stamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

pub fn bust(url: &str) -> String {
    let separator = if url.contains('?') { '&' } else { '?' };
    format!("{url}{separator}t={}", stamp())
}

pub struct Request<'a> {
    pub timeout: Duration,
    /// Додати `?t=<час>`, щоб обійти кеш CDN (для індексів і маніфестів).
    pub bust: bool,
    pub accept: Option<&'a str>,
}

impl Default for Request<'_> {
    fn default() -> Self {
        Self {
            timeout: JSON_TIMEOUT,
            bust: false,
            accept: None,
        }
    }
}

/// Перебирає готовий список адрес; `parse` перевіряє відповідь (сторінка-заглушка
/// провайдера замість JSON теж вважається збоєм дзеркала). Повертає результат і адресу, що відповіла.
pub async fn fetch_list<T>(
    client: &Client,
    urls: &[String],
    request: &Request<'_>,
    parse: impl Fn(&str) -> std::result::Result<T, String>,
) -> std::result::Result<(T, String), FetchError> {
    let mut not_found = false;
    let mut network_only = true;
    let mut last_error: Option<String> = None;

    for url in urls {
        let target = if request.bust { bust(url) } else { url.clone() };
        let mut builder = client.get(&target).timeout(request.timeout);
        if let Some(accept) = request.accept {
            builder = builder.header("Accept", accept);
        }
        let response = match builder.send().await {
            Ok(response) => response,
            Err(error) => {
                mark_down(url);
                if !is_network(&error) {
                    network_only = false;
                }
                last_error = Some(format!("{error}"));
                continue;
            }
        };
        let status = response.status();
        if !status.is_success() {
            network_only = false;
            if status == StatusCode::NOT_FOUND || status == StatusCode::GONE {
                not_found = true;
                continue;
            }
            if is_retryable(status) {
                if is_host_failure(status) {
                    mark_down(url);
                }
                last_error = Some(format!("HTTP {status}: {url}"));
                continue;
            }
            return Err(FetchError::Other(format!("HTTP {status}: {url}")));
        }
        let text = match response.text().await {
            Ok(text) => text,
            Err(error) => {
                mark_down(url);
                last_error = Some(format!("{error}"));
                continue;
            }
        };
        match parse(&text) {
            Ok(value) => {
                mark_up(url);
                return Ok((value, url.clone()));
            }
            Err(error) => {
                network_only = false;
                mark_down(url);
                last_error = Some(format!("некоректна відповідь ({url}): {error}"));
            }
        }
    }

    if not_found {
        Err(FetchError::NotFound)
    } else if network_only {
        Err(FetchError::Offline)
    } else {
        Err(FetchError::Other(
            last_error.unwrap_or_else(|| "no mirrors available".to_string()),
        ))
    }
}

/// JSON за посиланням з автоматичним перебором дзеркал.
pub async fn get_json<T: DeserializeOwned>(
    client: &Client,
    url: &str,
    bust: bool,
) -> std::result::Result<T, FetchError> {
    let urls = content_candidates(url);
    let request = Request {
        bust,
        ..Request::default()
    };
    fetch_list(client, &urls, &request, |text| {
        serde_json::from_str::<T>(text).map_err(|e| e.to_string())
    })
    .await
    .map(|(value, _)| value)
}

// ───────────────────────────── конфіг дзеркал ─────────────────────────────

fn parse_config(text: &str) -> std::result::Result<MirrorsConfig, String> {
    let config: MirrorsConfig = serde_json::from_str(text).map_err(|e| e.to_string())?;
    let useful = config
        .content
        .iter()
        .chain(config.update.iter())
        .any(|t| clean_template(t).is_some());
    if useful {
        Ok(config)
    } else {
        Err("no valid mirror templates".to_string())
    }
}

fn read_config(path: &Path) -> Option<MirrorsConfig> {
    let text = std::fs::read_to_string(path).ok()?;
    parse_config(&text).ok()
}

/// Під час запуску: власні дзеркала + кеш останнього віддаленого конфігу (без мережі).
pub fn init_from_disk(root: &Path) {
    let custom = read_config(&root.join(CUSTOM_FILE));
    let cached = read_config(&root.join(CACHE_FILE));
    let source = match (&custom, &cached) {
        (Some(_), _) => "custom",
        (None, Some(_)) => "cache",
        _ => "builtin",
    };
    apply(build(custom, cached, source));
}

/// Завантажує свіжий конфіг дзеркал і кешує його. Помилка не критична — лишаються кеш і вбудовані.
pub async fn refresh(client: &Client, root: &Path) -> bool {
    let Some(url) = MIRRORS_CONFIG_URL else {
        return false;
    };
    let urls = content_candidates(url);
    let request = Request {
        timeout: CONFIG_TIMEOUT,
        bust: true,
        accept: None,
    };
    let Ok((config, _)) = fetch_list(client, &urls, &request, parse_config).await else {
        return false;
    };
    if tokio::fs::create_dir_all(root).await.is_ok() {
        if let Ok(text) = serde_json::to_string_pretty(&config) {
            let temp = root.join("mirrors.json.tmp");
            if tokio::fs::write(&temp, text).await.is_ok() {
                let _ = tokio::fs::rename(&temp, root.join(CACHE_FILE)).await;
            }
        }
    }
    let custom = read_config(&root.join(CUSTOM_FILE));
    apply(build(custom, Some(config), "remote"));
    true
}

#[derive(Serialize)]
pub struct MirrorsInfo {
    pub content: Vec<String>,
    pub update: Vec<String>,
    /// builtin | cache | remote | custom
    pub source: String,
    pub down: Vec<String>,
    pub config_url: String,
    pub cache_file: String,
    pub custom_file: String,
}

pub fn info(root: &Path) -> MirrorsInfo {
    let set = active();
    MirrorsInfo {
        content: set.content.clone(),
        update: set.update.clone(),
        source: set.source.to_string(),
        down: down_hosts(),
        config_url: MIRRORS_CONFIG_URL.unwrap_or("").to_string(),
        cache_file: root.join(CACHE_FILE).to_string_lossy().to_string(),
        custom_file: root.join(CUSTOM_FILE).to_string_lossy().to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_repo_files() {
        let raw = repo_file("https://raw.githubusercontent.com/o/r/main/instances/a/manifest.json?t=1").unwrap();
        assert_eq!((raw.owner.as_str(), raw.repo.as_str(), raw.branch.as_str()), ("o", "r", "main"));
        assert_eq!(raw.path, "instances/a/manifest.json");
        let cdn = repo_file("https://cdn.jsdelivr.net/gh/o/r@dev/mods/x.jar").unwrap();
        assert_eq!((cdn.branch.as_str(), cdn.path.as_str()), ("dev", "mods/x.jar"));
        let gh = repo_file("https://github.com/o/r/raw/main/a.json").unwrap();
        assert_eq!(gh.path, "a.json");
        assert!(repo_file("https://github.com/o/r/releases/download/v1/a.exe").is_none());
        assert!(repo_file("https://example.com/a.json").is_none());
    }

    #[test]
    fn expands_candidates_in_order() {
        let list = content_candidates("https://raw.githubusercontent.com/o/r/main/mods/a%20b.jar");
        assert_eq!(list[0], "https://raw.githubusercontent.com/o/r/main/mods/a%20b.jar");
        assert!(list.contains(&"https://cdn.jsdelivr.net/gh/o/r@main/mods/a%20b.jar".to_string()));
        assert_eq!(list.iter().filter(|u| u.contains("raw.githubusercontent")).count(), 1);
        let other = content_candidates("https://example.com/file.bin");
        assert_eq!(other, vec!["https://example.com/file.bin".to_string()]);
    }

    #[test]
    fn proxy_templates_wrap_canonical_url() {
        let set = build(
            Some(MirrorsConfig {
                content: vec!["https://proxy.example/{url}".into(), "not a url".into()],
                update: vec![],
            }),
            None,
            "custom",
        );
        assert_eq!(set.content[0], "https://proxy.example/{url}");
        assert!(set.content.iter().all(|t| t.starts_with("https://")));
    }
}
