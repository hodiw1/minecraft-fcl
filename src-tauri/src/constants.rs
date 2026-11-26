pub const GITHUB_OWNER: &str = "hodiw1";
pub const GITHUB_REPO: &str = "minecraft-fcl";
pub const GITHUB_BRANCH: &str = "main";
pub const LAUNCHER_NAME: &str = "CustomLauncher";

pub fn manifest_url() -> String {
    format!("https://raw.githubusercontent.com/{GITHUB_OWNER}/{GITHUB_REPO}/{GITHUB_BRANCH}/manifest.json")
}

pub fn releases_api_url() -> String {
    format!("https://api.github.com/repos/{GITHUB_OWNER}/{GITHUB_REPO}/releases/latest")
}
