pub const GITHUB_OWNER: &str = "hodiw1";
pub const GITHUB_REPO: &str = "minecraft-fcl";
pub const GITHUB_BRANCH: &str = "main";
pub const LAUNCHER_NAME: &str = "CustomLauncher";

/// Файл-реєстр кодів проєктів у репозиторії лаунчера:
/// `{"projects": {"FNAF-2026": {"name": "...", "repo": "owner/repo", "branch": "main"}}}`
pub const REGISTRY_FILE: &str = "projects.json";
/// Індекс збірок усередині репозиторію проєкту.
pub const INDEX_FILE: &str = "instances.json";
/// Старий формат: одна збірка = manifest.json у корені репозиторію.
pub const LEGACY_MANIFEST_FILE: &str = "manifest.json";

pub const DEFAULT_GAME_LANG: &str = "uk_ua";
pub const DEFAULT_UI_LANG: &str = "uk";
pub const DEFAULT_PRESET: &str = "medium";

pub fn raw_url(owner: &str, repo: &str, branch: &str, path: &str) -> String {
    format!("https://raw.githubusercontent.com/{owner}/{repo}/{branch}/{path}")
}

pub fn registry_url() -> String {
    raw_url(GITHUB_OWNER, GITHUB_REPO, GITHUB_BRANCH, REGISTRY_FILE)
}

pub fn releases_api_url() -> String {
    format!("https://api.github.com/repos/{GITHUB_OWNER}/{GITHUB_REPO}/releases/latest")
}
