use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::constants::{DEFAULT_GAME_LANG, DEFAULT_PRESET, DEFAULT_UI_LANG};
use crate::error::{fail, Result};
use crate::i18n::tr;

pub const PRESETS: [&str; 4] = ["low", "medium", "high", "ultra"];
pub const UI_LANGS: [&str; 2] = ["uk", "en"];
pub const GAME_LANGS: [&str; 10] = [
    "uk_ua", "en_us", "en_gb", "pl_pl", "de_de", "fr_fr", "es_es", "cs_cz", "ru_ru", "be_by",
];

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

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Profile {
    pub id: String,
    pub nickname: String,
    #[serde(default)]
    pub has_skin: bool,
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
    /// Старе поле з версії 0.1 — переноситься у профілі й більше не зберігається.
    #[serde(rename = "nickname", default, skip_serializing)]
    pub legacy_nickname: String,
    #[serde(default)]
    pub profiles: Vec<Profile>,
    #[serde(default)]
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
}

impl Default for Config {
    fn default() -> Self {
        Self {
            legacy_nickname: String::new(),
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
        }
    }
}

impl Config {
    fn normalize(&mut self) {
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

pub async fn load(root: &Path) -> Config {
    let mut config = match tokio::fs::read_to_string(root.join("config.json")).await {
        Ok(text) => serde_json::from_str(&text).unwrap_or_default(),
        Err(_) => Config::default(),
    };
    config.normalize();
    config
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
