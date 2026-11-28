use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::constants::{DEFAULT_GAME_LANG, DEFAULT_PRESET, DEFAULT_UI_LANG};
use crate::error::{fail, AppError, Result};
use crate::i18n::tr;

pub const PRESETS: [&str; 4] = ["low", "medium", "high", "ultra"];
/// Кольорові палітри (акцент інтерфейсу). Самі кольори — у `ui/app.js` (THEMES).
pub const THEMES: [&str; 6] = ["emerald", "cobalt", "amber", "ruby", "amethyst", "graphite"];
pub const DEFAULT_THEME: &str = "emerald";
pub const UI_LANGS: [&str; 2] = ["uk", "en"];
pub const GAME_LANGS: [&str; 10] = [
    "uk_ua", "en_us", "en_gb", "pl_pl", "de_de", "fr_fr", "es_es", "cs_cz", "ru_ru", "be_by",
];

/// Версія формату config.json: 0 — до v0.4.1 (незалежні профілі), 2 — єдиний акаунт `@username`.
pub const CONFIG_SCHEMA: u32 = 2;
/// Скільки акаунтів, з яких вийшли, лаунчер пам'ятає на цьому ПК.
const MAX_REMEMBERED: usize = 10;

fn default_ram() -> u32 {
    4096
}

fn default_preset() -> String {
    DEFAULT_PRESET.to_string()
}

fn default_ui_lang() -> String {
    DEFAULT_UI_LANG.to_string()
}

fn default_game_lang() -> String {
    DEFAULT_GAME_LANG.to_string()
}

fn default_theme() -> String {
    DEFAULT_THEME.to_string()
}

/// Ігровий нік усередині акаунта Terrapolis (з власним скіном) — під ним гравець заходить
/// у Minecraft. До v0.4.1 це були окремі «профілі»; формат запису той самий, тож старі ніки
/// та скіни (`skins/<id>.png`) переносяться без змін.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Profile {
    pub id: String,
    pub nickname: String,
    #[serde(default)]
    pub has_skin: bool,
}

/// Акаунт, з якого гравець вийшов на цьому комп'ютері: токен і ніки лишаються локально,
/// тож повторний вхід з тим самим `@username` повертає все як було.
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct RememberedAccount {
    pub username: String,
    #[serde(default)]
    pub account_id: String,
    pub account_token: String,
    #[serde(default)]
    pub characters: Vec<Profile>,
    #[serde(default)]
    pub active_character: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ProjectBinding {
    /// Код, який увів гравець (як показувати в інтерфейсі).
    pub code: String,
    #[serde(default)]
    pub name: String,
    /// Повне https-посилання на instances.json (або manifest.json старого формату).
    pub index_url: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Config {
    /// Старе поле з версії 0.1 — переноситься у ніки й більше не зберігається.
    #[serde(rename = "nickname", default, skip_serializing)]
    pub legacy_nickname: String,
    /// Формат файлу (див. [`CONFIG_SCHEMA`]). Відсутнє поле = конфіг до v0.4.1.
    #[serde(default)]
    pub schema: u32,
    /// UUID акаунта в Supabase (порожньо — акаунт створено офлайн і ще не зареєстровано).
    #[serde(default)]
    pub account_id: String,
    /// Унікальний логін лаунчера `@username` (3–20 символів `[A-Za-z0-9_]`).
    #[serde(default)]
    pub username: String,
    /// Випадковий 32-байтний секрет (hex). Сервер зберігає лише його SHA-256.
    #[serde(default)]
    pub account_token: String,
    /// `@username` підтверджено сервером (`register_account`).
    #[serde(default)]
    pub account_synced: bool,
    /// Офлайн-створений `@username` виявився зайнятим — треба обрати інший.
    #[serde(default)]
    pub account_conflict: bool,
    /// Акаунти, з яких вийшли на цьому ПК.
    #[serde(default)]
    pub remembered: Vec<RememberedAccount>,
    /// Ігрові ніки та скіни акаунта. До v0.4.1 поле називалося `profiles`.
    #[serde(rename = "characters", alias = "profiles", default)]
    pub profiles: Vec<Profile>,
    #[serde(rename = "active_character", alias = "active_profile", default)]
    pub active_profile: String,
    #[serde(default = "default_ram")]
    pub ram_mb: u32,
    #[serde(default)]
    pub project: Option<ProjectBinding>,
    #[serde(default)]
    pub active_instance: String,
    /// Порожньо = стандартна папка лаунчера (AppData).
    #[serde(default)]
    pub data_dir: String,
    #[serde(default = "default_preset")]
    pub graphics_preset: String,
    #[serde(default = "default_ui_lang")]
    pub ui_lang: String,
    #[serde(default = "default_game_lang")]
    pub game_lang: String,
    /// Автоматично завантажувати й встановлювати оновлення лаунчера (інакше — лише сповіщення).
    #[serde(default)]
    pub auto_update_launcher: bool,
    /// Ключ CurseForge Core API (https://console.curseforge.com). Без нього працює лише Modrinth.
    #[serde(default)]
    pub curseforge_api_key: String,
    /// Акцентна палітра інтерфейсу (див. [`THEMES`]).
    #[serde(default = "default_theme")]
    pub theme: String,
    /// Гравець пройшов майстер першого запуску (@username + нік + палітра → проєкт або «Пропустити»).
    #[serde(default)]
    pub onboarded: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            legacy_nickname: String::new(),
            schema: CONFIG_SCHEMA,
            account_id: String::new(),
            username: String::new(),
            account_token: String::new(),
            account_synced: false,
            account_conflict: false,
            remembered: Vec::new(),
            profiles: Vec::new(),
            active_profile: String::new(),
            ram_mb: default_ram(),
            project: None,
            active_instance: String::new(),
            data_dir: String::new(),
            graphics_preset: default_preset(),
            ui_lang: default_ui_lang(),
            game_lang: default_game_lang(),
            auto_update_launcher: false,
            curseforge_api_key: String::new(),
            theme: default_theme(),
            onboarded: false,
        }
    }
}

impl Config {
    pub(crate) fn normalize(&mut self) {
        let legacy = std::mem::take(&mut self.legacy_nickname);
        let legacy = legacy.trim();
        if self.profiles.is_empty() && validate_nickname(legacy).is_ok() {
            self.profiles.push(Profile {
                id: new_id(),
                nickname: legacy.to_string(),
                has_skin: false,
            });
        }
        if !self.profiles.iter().any(|p| p.id == self.active_profile) {
            self.active_profile = self
                .profiles
                .first()
                .map(|p| p.id.clone())
                .unwrap_or_default();
        }
        if !PRESETS.contains(&self.graphics_preset.as_str()) {
            self.graphics_preset = default_preset();
        }
        if !UI_LANGS.contains(&self.ui_lang.as_str()) {
            self.ui_lang = default_ui_lang();
        }
        if !GAME_LANGS.contains(&self.game_lang.as_str()) {
            self.game_lang = default_game_lang();
        }
        if !THEMES.contains(&self.theme.as_str()) {
            self.theme = default_theme();
        }
        // Конфіг до v0.4.1 (міграція при оновленні): у кого вже є нік, той повний майстер
        // не бачить — лише коротке вікно вибору @username (`needs_username`). Ніки, скіни,
        // палітра, налаштування, збірки й проєкт лишаються як були.
        if self.schema < CONFIG_SCHEMA {
            if !self.profiles.is_empty() {
                self.onboarded = true;
            }
            self.schema = CONFIG_SCHEMA;
        }
        if validate_username(&self.username).is_err() {
            self.username.clear();
            self.account_synced = false;
        }
        if !is_token(&self.account_token) {
            self.account_token.clear();
            self.account_synced = false;
        }
        if !self.account_synced {
            self.account_id.clear();
        }
        self.remembered
            .retain(|r| validate_username(&r.username).is_ok() && is_token(&r.account_token));
    }

    /// Гравець уже користувався лаунчером, але ще не має `@username` (оновлення з v0.4.0)
    /// або офлайн-створене ім'я виявилося зайнятим.
    pub fn needs_username(&self) -> bool {
        self.onboarded && (self.username.is_empty() || self.account_conflict)
    }

    /// `(account_id, account_token)`, якщо акаунт зареєстровано на сервері.
    pub fn online_account(&self) -> Option<(String, String)> {
        if self.account_synced && !self.account_id.is_empty() && is_token(&self.account_token) {
            Some((self.account_id.clone(), self.account_token.clone()))
        } else {
            None
        }
    }

    /// «Вийти з акаунта»: токен, id і ніки переносяться в `remembered` (повторний вхід з тим
    /// самим @username на цьому ПК поверне все), поточний акаунт очищається, і лаунчер
    /// повертається до майстра першого запуску. Збірки, проєкт і налаштування не чіпаємо.
    pub fn sign_out(&mut self) {
        if !self.username.is_empty() && is_token(&self.account_token) {
            let username = self.username.clone();
            self.remembered
                .retain(|r| !r.username.eq_ignore_ascii_case(&username));
            self.remembered.push(RememberedAccount {
                username,
                account_id: if self.account_synced {
                    self.account_id.clone()
                } else {
                    String::new()
                },
                account_token: self.account_token.clone(),
                characters: std::mem::take(&mut self.profiles),
                active_character: std::mem::take(&mut self.active_profile),
            });
            let excess = self.remembered.len().saturating_sub(MAX_REMEMBERED);
            self.remembered.drain(..excess);
        }
        self.account_id.clear();
        self.username.clear();
        self.account_token.clear();
        self.account_synced = false;
        self.account_conflict = false;
        self.profiles.clear();
        self.active_profile.clear();
        self.onboarded = false;
    }

    /// Забирає з `remembered` акаунт із цим @username (повторний вхід на цьому ПК).
    pub fn take_remembered(&mut self, username: &str) -> Option<RememberedAccount> {
        let index = self
            .remembered
            .iter()
            .position(|r| r.username.eq_ignore_ascii_case(username))?;
        Some(self.remembered.remove(index))
    }

    /// Скидання «технічних» налаштувань (Danger Zone): лишаються акаунт, ніки, проєкт,
    /// мова інтерфейсу та палітра — усе інше повертається до стандартних значень.
    pub fn reset_technical(&self) -> Config {
        Config {
            account_id: self.account_id.clone(),
            username: self.username.clone(),
            account_token: self.account_token.clone(),
            account_synced: self.account_synced,
            account_conflict: self.account_conflict,
            remembered: self.remembered.clone(),
            profiles: self.profiles.clone(),
            active_profile: self.active_profile.clone(),
            project: self.project.clone(),
            ui_lang: self.ui_lang.clone(),
            theme: self.theme.clone(),
            onboarded: self.onboarded,
            ..Config::default()
        }
    }

    pub fn active_profile(&self) -> Option<&Profile> {
        self.profiles.iter().find(|p| p.id == self.active_profile)
    }

    /// Ключ CurseForge: з налаштувань, а якщо там порожньо — зі змінної середовища.
    pub fn curseforge_key(&self) -> Option<String> {
        let saved = self.curseforge_api_key.trim();
        if !saved.is_empty() {
            return Some(saved.to_string());
        }
        ["TERRAPOLIS_CURSEFORGE_KEY", "CURSEFORGE_API_KEY"]
            .iter()
            .filter_map(|name| std::env::var(name).ok())
            .map(|value| value.trim().to_string())
            .find(|value| !value.is_empty())
    }
}

/// Ключ API: друковані ASCII-символи без пробілів (інакше заголовок HTTP буде некоректним).
pub fn validate_api_key(key: &str) -> Result<()> {
    if key.len() > 200 || !key.chars().all(|c| c.is_ascii_graphic()) {
        return fail(tr(
            "Ключ CurseForge виглядає некоректно — скопіюй його ще раз без пробілів",
            "The CurseForge key looks invalid — copy it again without spaces",
        ));
    }
    Ok(())
}

pub fn new_id() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("p{:x}", nanos)
}

fn is_token(token: &str) -> bool {
    token.len() == 64 && token.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

/// Новий секрет акаунта: 32 випадкові байти з генератора ОС (hex, 64 символи).
pub fn new_account_token() -> Result<String> {
    let mut bytes = [0u8; 32];
    getrandom::getrandom(&mut bytes).map_err(|e| AppError(format!("random: {e}")))?;
    Ok(hex::encode(bytes))
}

/// SHA-256 токена (hex) — саме його отримує сервер під час реєстрації.
pub fn token_hash(token: &str) -> String {
    use sha2::{Digest, Sha256};
    hex::encode(Sha256::digest(token.as_bytes()))
}

/// `@username`: 3–20 символів — латиниця, цифри та `_` (унікальний без урахування регістру).
pub fn validate_username(username: &str) -> Result<()> {
    let length = username.chars().count();
    if !(3..=20).contains(&length) {
        return fail(tr(
            "@username має містити від 3 до 20 символів",
            "@username must be 3 to 20 characters long",
        ));
    }
    if !username.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return fail(tr(
            "@username може містити лише латинські літери, цифри та символ _",
            "@username may contain only Latin letters, digits and _",
        ));
    }
    Ok(())
}

/// Підказка для вікна оновлення: поточний основний нік (правила ніка — підмножина правил @username).
pub fn suggested_username(config: &Config) -> String {
    let candidate = config
        .active_profile()
        .or_else(|| config.profiles.first())
        .map(|p| p.nickname.clone())
        .unwrap_or_default();
    if validate_username(&candidate).is_ok() {
        candidate
    } else {
        String::new()
    }
}

pub fn parse(text: &str) -> Config {
    // Пошкоджений config.json (обірваний запис, BOM) не валить лаунчер — стартуємо зі стандартних.
    let mut config: Config =
        serde_json::from_str(text.trim_start_matches('\u{feff}')).unwrap_or_default();
    config.normalize();
    config
}

pub async fn load(root: &Path) -> Config {
    match tokio::fs::read_to_string(root.join("config.json")).await {
        Ok(text) => parse(&text),
        Err(_) => {
            let mut config = Config::default();
            config.normalize();
            config
        }
    }
}

pub async fn save(root: &Path, config: &Config) -> Result<()> {
    tokio::fs::create_dir_all(root).await?;
    let text = serde_json::to_string_pretty(config)?;
    let target = root.join("config.json");
    let temp = root.join("config.json.tmp");
    tokio::fs::write(&temp, text).await?;
    tokio::fs::rename(&temp, &target).await?;
    Ok(())
}

pub fn validate_nickname(nickname: &str) -> Result<()> {
    let length = nickname.chars().count();
    if !(3..=16).contains(&length) {
        return fail(tr(
            "Нікнейм має містити від 3 до 16 символів",
            "Nickname must be 3 to 16 characters long",
        ));
    }
    if !nickname
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_')
    {
        return fail(tr(
            "Нікнейм може містити лише латинські літери, цифри та символ _",
            "Nickname may contain only Latin letters, digits and _",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Реальний config.json лаунчера v0.4.0: два профілі (один зі скіном), палітра,
    /// підключений проєкт, вибрана власна збірка, нестандартні налаштування.
    const V040_CONFIG: &str = r#"{
      "profiles": [
        {"id": "p17a2b", "nickname": "Steve_UA", "has_skin": true},
        {"id": "p17a2c", "nickname": "Alex", "has_skin": false}
      ],
      "active_profile": "p17a2c",
      "ram_mb": 8192,
      "project": {"code": "TERRA", "name": "Terrapolis SMP", "index_url": "https://raw.githubusercontent.com/hodiw1/terrapolis-smp/main/instances.json"},
      "active_instance": "my-18f00aa",
      "data_dir": "D:\\Games\\Terrapolis",
      "graphics_preset": "high",
      "ui_lang": "en",
      "game_lang": "pl_pl",
      "auto_update_launcher": true,
      "curseforge_api_key": "secret-key",
      "theme": "amethyst",
      "onboarded": true
    }"#;

    #[test]
    fn migration_from_v040_keeps_everything_and_asks_for_username() {
        let config = parse(V040_CONFIG);
        // Нічого не втрачено.
        assert_eq!(config.profiles.len(), 2);
        assert_eq!(config.profiles[0].nickname, "Steve_UA");
        assert!(config.profiles[0].has_skin);
        assert_eq!(config.profiles[1].nickname, "Alex");
        assert_eq!(config.active_profile, "p17a2c");
        assert_eq!(config.theme, "amethyst");
        assert_eq!(config.project.as_ref().map(|p| p.code.as_str()), Some("TERRA"));
        assert_eq!(config.active_instance, "my-18f00aa");
        assert_eq!(config.ram_mb, 8192);
        assert_eq!(config.data_dir, "D:\\Games\\Terrapolis");
        assert_eq!(config.graphics_preset, "high");
        assert_eq!(config.ui_lang, "en");
        assert_eq!(config.game_lang, "pl_pl");
        assert!(config.auto_update_launcher);
        assert_eq!(config.curseforge_api_key, "secret-key");
        // Повного майстра немає — лише вибір @username з підказкою від основного ніка.
        assert!(config.onboarded);
        assert!(config.needs_username());
        assert_eq!(suggested_username(&config), "Alex");
        assert_eq!(config.schema, CONFIG_SCHEMA);

        // Новий формат: ніки зберігаються як `characters`, і після повторного читання все на місці.
        let saved = serde_json::to_string(&config).unwrap();
        assert!(saved.contains("\"characters\""));
        assert!(saved.contains("\"active_character\""));
        assert!(!saved.contains("\"profiles\""));
        let reloaded = parse(&saved);
        assert_eq!(reloaded.profiles.len(), 2);
        assert_eq!(reloaded.active_profile, "p17a2c");
        assert!(reloaded.profiles[0].has_skin);
        assert_eq!(reloaded.theme, "amethyst");
        assert!(reloaded.project.is_some());
        assert!(reloaded.needs_username());
    }

    #[test]
    fn v040_profiles_without_onboarded_flag_still_skip_the_wizard() {
        // v0.3.x: профіль є, прапорця onboarded ще не існувало.
        let config = parse(r#"{"profiles":[{"id":"p1","nickname":"Steve"}]}"#);
        assert!(config.onboarded);
        assert!(config.needs_username());
        assert_eq!(config.theme, DEFAULT_THEME);
    }

    #[test]
    fn legacy_single_nickname_is_migrated() {
        let config = parse(r#"{"nickname":"OldPlayer"}"#);
        assert_eq!(config.profiles.len(), 1);
        assert_eq!(config.profiles[0].nickname, "OldPlayer");
        assert!(config.needs_username());
    }

    #[test]
    fn fresh_install_shows_full_onboarding() {
        let config = parse("{}");
        assert!(!config.onboarded);
        assert!(!config.needs_username());
    }

    #[test]
    fn unknown_theme_falls_back() {
        let config = parse(r#"{"theme":"neon-pink"}"#);
        assert_eq!(config.theme, DEFAULT_THEME);
        assert!(!config.onboarded);
    }

    fn account() -> Config {
        let mut config = parse(V040_CONFIG);
        config.username = "SteveUA".into();
        config.account_id = "8a6e0804-2bd0-4672-b79d-d97027f9071a".into();
        config.account_token = "ab".repeat(32);
        config.account_synced = true;
        config
    }

    #[test]
    fn signed_in_v041_config_round_trips() {
        let config = account();
        let reloaded = parse(&serde_json::to_string(&config).unwrap());
        assert!(!reloaded.needs_username());
        assert_eq!(reloaded.username, "SteveUA");
        assert!(reloaded.online_account().is_some());
        assert_eq!(reloaded.profiles.len(), 2);
    }

    #[test]
    fn sign_out_returns_to_onboarding_and_remembers_the_account() {
        let mut config = account();
        config.sign_out();
        assert!(config.username.is_empty());
        assert!(config.account_id.is_empty());
        assert!(config.account_token.is_empty());
        assert!(!config.onboarded);
        assert!(config.profiles.is_empty());
        // Збірки, проєкт і налаштування лишаються.
        assert!(config.project.is_some());
        assert_eq!(config.active_instance, "my-18f00aa");
        assert_eq!(config.theme, "amethyst");

        // Після перезапуску лаунчер не «повертає» онбординг через старі ніки.
        let reloaded = parse(&serde_json::to_string(&config).unwrap());
        assert!(!reloaded.onboarded);
        assert!(!reloaded.needs_username());

        // Повторний вхід з тим самим @username (будь-який регістр) повертає токен і ніки.
        let mut reloaded = reloaded;
        let back = reloaded.take_remembered("steveua").unwrap();
        assert_eq!(back.account_token, "ab".repeat(32));
        assert_eq!(back.characters.len(), 2);
        assert_eq!(back.active_character, "p17a2c");
        assert!(reloaded.remembered.is_empty());
    }

    #[test]
    fn broken_account_fields_are_dropped() {
        let config = parse(
            r#"{"schema":2,"onboarded":true,"characters":[{"id":"p1","nickname":"Steve"}],
                "username":"bad name!","account_token":"short","account_synced":true,"account_id":"x"}"#,
        );
        assert!(config.username.is_empty());
        assert!(config.account_token.is_empty());
        assert!(!config.account_synced);
        assert!(config.account_id.is_empty());
        assert!(config.needs_username());
    }

    #[test]
    fn usernames_are_validated() {
        for ok in ["abc", "Steve_UA", "a_very_long_name_20c"] {
            assert!(validate_username(ok).is_ok(), "{ok}");
        }
        for bad in ["ab", "with space", "кирилиця", "a_very_long_name_21ch", "@steve", ""] {
            assert!(validate_username(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn tokens_are_random_hex_and_hashed() {
        let a = new_account_token().unwrap();
        let b = new_account_token().unwrap();
        assert!(is_token(&a) && is_token(&b));
        assert_ne!(a, b);
        let hash = token_hash(&a);
        assert_eq!(hash.len(), 64);
        assert_ne!(hash, a);
        assert_eq!(
            token_hash("abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn technical_reset_keeps_identity() {
        let mut config = account();
        config.data_dir = "D:\\Games".into();
        config.theme = "ruby".into();
        let reset = config.reset_technical();
        assert_eq!(reset.profiles.len(), 2);
        assert_eq!(reset.active_profile, "p17a2c");
        assert!(reset.project.is_some());
        assert_eq!(reset.theme, "ruby");
        assert_eq!(reset.username, "SteveUA");
        assert!(reset.online_account().is_some());
        assert_eq!(reset.ram_mb, 4096);
        assert!(reset.data_dir.is_empty());
        assert!(reset.curseforge_api_key.is_empty());
    }
}
