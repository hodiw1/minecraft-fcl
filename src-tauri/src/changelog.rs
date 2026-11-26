use std::time::{Duration, SystemTime};

use futures_util::future::{join, try_join};
use reqwest::{Client, StatusCode};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::config::ProjectBinding;
use crate::constants::{GITHUB_OWNER, GITHUB_REPO};
use crate::error::{fail, Result};
use crate::i18n::tr;
use crate::paths::Paths;
use crate::project;

const API_TIMEOUT: Duration = Duration::from_secs(15);
const CACHE_SECS: u64 = 300;
const MAX_ITEMS: usize = 30;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ChangeItem {
    pub kind: String,
    pub tag: String,
    pub title: String,
    pub body: String,
    pub author: String,
    pub date: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct ChangeColumn {
    pub repo: String,
    pub items: Vec<ChangeItem>,
    pub error: String,
}

#[derive(Serialize)]
pub struct Changelog {
    pub launcher: ChangeColumn,
    pub project: ChangeColumn,
}

#[derive(Deserialize)]
struct ApiUser {
    login: String,
}

#[derive(Deserialize)]
struct ApiRelease {
    tag_name: String,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    body: Option<String>,
    #[serde(default)]
    published_at: Option<String>,
    #[serde(default)]
    created_at: Option<String>,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    author: Option<ApiUser>,
}

#[derive(Deserialize)]
struct ApiCommitAuthor {
    #[serde(default)]
    name: String,
    #[serde(default)]
    date: String,
}

#[derive(Deserialize)]
struct ApiCommitInner {
    message: String,
    #[serde(default)]
    author: Option<ApiCommitAuthor>,
}

#[derive(Deserialize)]
struct ApiCommit {
    sha: String,
    commit: ApiCommitInner,
    #[serde(default)]
    author: Option<ApiUser>,
}

fn clip(text: &str, limit: usize) -> String {
    text.trim().chars().take(limit).collect()
}

async fn api_list<T: DeserializeOwned>(client: &Client, url: &str) -> Result<Vec<T>> {
    let response = client
        .get(url)
        .header("Accept", "application/vnd.github+json")
        .timeout(API_TIMEOUT)
        .send()
        .await?;
    let status = response.status();
    if status == StatusCode::FORBIDDEN || status == StatusCode::TOO_MANY_REQUESTS {
        return fail(tr(
            "GitHub тимчасово обмежив кількість запитів. Спробуй за кілька хвилин.",
            "GitHub is temporarily rate limiting requests. Try again in a few minutes.",
        ));
    }
    if status == StatusCode::NOT_FOUND || status == StatusCode::CONFLICT {
        return Ok(Vec::new());
    }
    let response = response.error_for_status()?;
    Ok(response.json::<Vec<T>>().await?)
}

fn release_item(release: ApiRelease) -> Option<ChangeItem> {
    if release.draft {
        return None;
    }
    let title = release
        .name
        .as_deref()
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .unwrap_or(release.tag_name.as_str())
        .to_string();
    Some(ChangeItem {
        kind: "release".to_string(),
        tag: release.tag_name.clone(),
        title: clip(&title, 140),
        body: clip(release.body.as_deref().unwrap_or(""), 500),
        author: release.author.map(|user| user.login).unwrap_or_default(),
        date: release
            .published_at
            .or(release.created_at)
            .unwrap_or_default(),
    })
}

fn commit_item(commit: ApiCommit) -> ChangeItem {
    let message = commit.commit.message.trim().to_string();
    let (first, rest) = match message.split_once('\n') {
        Some((first, rest)) => (first.to_string(), rest.trim().to_string()),
        None => (message, String::new()),
    };
    let (name, date) = match commit.commit.author {
        Some(author) => (author.name, author.date),
        None => (String::new(), String::new()),
    };
    ChangeItem {
        kind: "commit".to_string(),
        tag: commit.sha.chars().take(7).collect(),
        title: clip(&first, 140),
        body: clip(&rest, 300),
        author: commit.author.map(|user| user.login).unwrap_or(name),
        date,
    }
}

async fn fetch_items(client: &Client, owner: &str, repo: &str) -> Result<Vec<ChangeItem>> {
    let releases_url = format!("https://api.github.com/repos/{owner}/{repo}/releases?per_page=10");
    let commits_url = format!("https://api.github.com/repos/{owner}/{repo}/commits?per_page=20");
    let (releases, commits) = try_join(
        api_list::<ApiRelease>(client, &releases_url),
        api_list::<ApiCommit>(client, &commits_url),
    )
    .await?;

    let mut items: Vec<ChangeItem> = releases.into_iter().filter_map(release_item).collect();
    items.extend(commits.into_iter().map(commit_item));
    items.sort_by(|a, b| b.date.cmp(&a.date));
    items.truncate(MAX_ITEMS);
    Ok(items)
}

fn cache_path(paths: &Paths, owner: &str, repo: &str) -> std::path::PathBuf {
    paths.projects_cache().join(format!(
        "changelog-{}--{}.json",
        owner.to_lowercase(),
        repo.to_lowercase()
    ))
}

async fn read_cache(path: &std::path::Path) -> Option<Vec<ChangeItem>> {
    let text = tokio::fs::read_to_string(path).await.ok()?;
    serde_json::from_str(&text).ok()
}

async fn read_fresh_cache(path: &std::path::Path) -> Option<Vec<ChangeItem>> {
    let modified = tokio::fs::metadata(path).await.ok()?.modified().ok()?;
    let age = SystemTime::now().duration_since(modified).ok()?;
    if age.as_secs() >= CACHE_SECS {
        return None;
    }
    read_cache(path).await
}

async fn column(client: &Client, paths: &Paths, owner: &str, repo: &str) -> ChangeColumn {
    let label = format!("{owner}/{repo}");
    let path = cache_path(paths, owner, repo);
    if let Some(items) = read_fresh_cache(&path).await {
        return ChangeColumn {
            repo: label,
            items,
            error: String::new(),
        };
    }
    match fetch_items(client, owner, repo).await {
        Ok(items) => {
            if let Some(parent) = path.parent() {
                let _ = tokio::fs::create_dir_all(parent).await;
            }
            if let Ok(text) = serde_json::to_string(&items) {
                let _ = tokio::fs::write(&path, text).await;
            }
            ChangeColumn {
                repo: label,
                items,
                error: String::new(),
            }
        }
        Err(error) => match read_cache(&path).await {
            Some(items) => ChangeColumn {
                repo: label,
                items,
                error: String::new(),
            },
            None => ChangeColumn {
                repo: label,
                items: Vec::new(),
                error: error.0,
            },
        },
    }
}

pub async fn load(client: &Client, paths: &Paths, binding: Option<&ProjectBinding>) -> Changelog {
    let source = binding.and_then(|binding| project::github_source(&binding.index_url));
    let project_column = async {
        match source {
            Some((owner, repo, _)) => column(client, paths, &owner, &repo).await,
            None => ChangeColumn::default(),
        }
    };
    let (launcher, project) = join(
        column(client, paths, GITHUB_OWNER, GITHUB_REPO),
        project_column,
    )
    .await;
    Changelog { launcher, project }
}
