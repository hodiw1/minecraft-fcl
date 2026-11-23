use serde::{Deserialize, Serialize};

use crate::error::{fail, Result};
use crate::paths::Paths;

fn default_ram() -> u32 {
    4096
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Config {
    #[serde(default)]
    pub nickname: String,
    #[serde(default = "default_ram")]
    pub ram_mb: u32,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            nickname: String::new(),
            ram_mb: default_ram(),
        }
    }
}

pub async fn load(paths: &Paths) -> Config {
    match tokio::fs::read_to_string(&paths.config_file).await {
        Ok(text) => serde_json::from_str(&text).unwrap_or_default(),
        Err(_) => Config::default(),
    }
}

pub async fn save(paths: &Paths, config: &Config) -> Result<()> {
    tokio::fs::create_dir_all(&paths.root).await?;
    let text = serde_json::to_string_pretty(config)?;
    tokio::fs::write(&paths.config_file, text).await?;
    Ok(())
}

pub fn validate_nickname(nickname: &str) -> Result<()> {
    let length = nickname.chars().count();
    if !(3..=16).contains(&length) {
        return fail("Нікнейм має містити від 3 до 16 символів");
    }
    if !nickname
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_')
    {
        return fail("Нікнейм може містити лише латинські літери, цифри та символ _");
    }
    Ok(())
}
