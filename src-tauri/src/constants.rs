pub const GITHUB_OWNER: &str = "hodiw1";
pub const GITHUB_REPO: &str = "terrapolis";
pub const GITHUB_BRANCH: &str = "main";
pub const LAUNCHER_NAME: &str = "CustomLauncher";

pub const PROJECT_PREFIX: &str = "terrapolis-";
pub const TRUSTED_OWNERS: [&str; 1] = [GITHUB_OWNER];
pub const PROJECT_LIST_TTL_SECS: u64 = 120;

/// Файл-реєстр кодів проєктів у репозиторії лаунчера:
/// `{"projects": {"MY-SERVER": {"name": "...", "repo": "owner/repo", "branch": "main"}}}`
pub const REGISTRY_FILE: &str = "projects.json";
/// Індекс збірок усередині репозиторію проєкту.
pub const INDEX_FILE: &str = "instances.json";
/// Старий формат: одна збірка = manifest.json у корені репозиторію.
pub const LEGACY_MANIFEST_FILE: &str = "manifest.json";

pub const DEFAULT_GAME_LANG: &str = "uk_ua";
pub const DEFAULT_UI_LANG: &str = "uk";
pub const DEFAULT_PRESET: &str = "medium";

/// CurseForge у каталозі контенту (моди, ресурс-паки, шейдери, готові збірки).
/// Працює з API-ключем гравця (Налаштування → «Джерела модів» або `CURSEFORGE_API_KEY`);
/// без ключа інтерфейс показує підказку, а Modrinth працює як завжди.
/// Щоб приховати платформу, досить змінити прапорець на `false` (і `ENABLE_CURSEFORGE` у `ui/app.js`).
pub const ENABLE_CURSEFORGE: bool = true;

// ───────────────────────────── дзеркала ─────────────────────────────
//
// Шаблон дзеркала — https-адреса з плейсхолдерами:
//   {owner} {repo} {branch} — частини оригінального посилання на файл у GitHub-репозиторії;
//                            шлях до файлу дописується в кінець шаблону;
//   {url}                   — повне оригінальне посилання (для проксі на кшталт
//                            `https://proxy.example.com/{url}`; працює і для github.com/releases).
// Порядок важливий: першим завжди пробується оригінальне посилання, далі — дзеркала за списком.

/// Дзеркала контенту: projects.json, instances.json, manifest.json і файли збірок.
pub const CONTENT_MIRRORS: &[&str] = &[
    "https://raw.githubusercontent.com/{owner}/{repo}/{branch}",
    "https://cdn.jsdelivr.net/gh/{owner}/{repo}@{branch}",
    "https://fastly.jsdelivr.net/gh/{owner}/{repo}@{branch}",
];

/// Джерела інформації про нову версію лаунчера. Відповідь — або реліз GitHub
/// (`tag_name`, `body`, `assets`), або `update.json` (`version`, `notes`, `url`, `sha256`).
pub const UPDATE_MIRRORS: &[&str] = &[
    "https://api.github.com/repos/{owner}/{repo}/releases/latest",
    "https://raw.githubusercontent.com/{owner}/{repo}/{branch}/update.json",
    "https://cdn.jsdelivr.net/gh/{owner}/{repo}@{branch}/update.json",
];

/// Віддалений JSON з актуальним переліком дзеркал (`{"content": [...], "update": [...]}`).
/// Сам завантажується через CONTENT_MIRRORS, після успіху кешується в `<AppData>/mirrors.json`.
/// `None` — користуватися лише вбудованими дзеркалами.
pub const MIRRORS_CONFIG_URL: Option<&str> =
    Some("https://raw.githubusercontent.com/hodiw1/terrapolis/main/mirrors.json");

pub fn raw_url(owner: &str, repo: &str, branch: &str, path: &str) -> String {
    format!("https://raw.githubusercontent.com/{owner}/{repo}/{branch}/{path}")
}

pub fn registry_url() -> String {
    raw_url(GITHUB_OWNER, GITHUB_REPO, GITHUB_BRANCH, REGISTRY_FILE)
}
