use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::paths::Paths;

/// Те, що лаунчер ніколи не видаляє і не перезаписує під час перевірки/ремонту:
/// налаштування гравця, світи, скріншоти, мітки карт.
pub const PROTECTED: [&str; 12] = [
    "options.txt",
    "optionsof.txt",
    "optionsshaders.txt",
    "saves",
    "screenshots",
    "xaero",
    "XaeroWaypoints",
    "XaeroWorldMap",
    "journeymap",
    "logs",
    "crash-reports",
    "CustomSkinLoader",
];

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct InstanceState {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub project: String,
    /// Версія збірки (з manifest.json), яку повністю встановлено.
    #[serde(default)]
    pub pack_version: String,
    #[serde(default)]
    pub installed: bool,
    /// path -> sha256, який лаунчер записав на диск. Для конфігів — щоб відрізнити
    /// "адмін оновив файл" від "гравець змінив файл".
    #[serde(default)]
    pub files: HashMap<String, String>,
    #[serde(default)]
    pub preset_applied: bool,
}

pub fn state_file(paths: &Paths, id: &str) -> PathBuf {
    paths.instance_meta(id).join("state.json")
}

pub fn manifest_cache(paths: &Paths, id: &str) -> PathBuf {
    paths.instance_meta(id).join("manifest.json")
}

pub fn game_log(paths: &Paths, id: &str) -> PathBuf {
    paths.instance_meta(id).join("game_output.log")
}

pub async fn load_state(paths: &Paths, id: &str) -> InstanceState {
    match tokio::fs::read_to_string(state_file(paths, id)).await {
        Ok(text) => serde_json::from_str(&text).unwrap_or_default(),
        Err(_) => InstanceState::default(),
    }
}

pub async fn save_state(paths: &Paths, id: &str, state: &InstanceState) -> Result<()> {
    let meta = paths.instance_meta(id);
    tokio::fs::create_dir_all(&meta).await?;
    let temp = meta.join("state.json.tmp");
    tokio::fs::write(&temp, serde_json::to_string_pretty(state)?).await?;
    tokio::fs::rename(&temp, state_file(paths, id)).await?;
    Ok(())
}

pub async fn ensure_dirs(paths: &Paths, id: &str) -> Result<()> {
    let dir = paths.instance_dir(id);
    for child in [
        "mods",
        "config",
        "resourcepacks",
        "shaderpacks",
        ".launcher",
    ] {
        tokio::fs::create_dir_all(dir.join(child)).await?;
    }
    Ok(())
}

pub fn dir_size(path: &Path) -> u64 {
    let mut total = 0u64;
    let mut stack = vec![path.to_path_buf()];
    while let Some(current) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&current) else {
            continue;
        };
        for entry in entries.flatten() {
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            if kind.is_dir() {
                stack.push(entry.path());
            } else if kind.is_file() {
                total += entry.metadata().map(|m| m.len()).unwrap_or(0);
            }
        }
    }
    total
}

#[derive(Serialize, Clone, Debug)]
pub struct DiskInstance {
    pub id: String,
    pub name: String,
    pub project: String,
    pub pack_version: String,
    pub installed: bool,
    pub size_bytes: u64,
}

pub async fn list_on_disk(paths: &Paths) -> Vec<DiskInstance> {
    let mut out = Vec::new();
    let Ok(mut entries) = tokio::fs::read_dir(&paths.instances).await else {
        return out;
    };
    while let Ok(Some(entry)) = entries.next_entry().await {
        let Ok(kind) = entry.file_type().await else {
            continue;
        };
        if !kind.is_dir() {
            continue;
        }
        let id = entry.file_name().to_string_lossy().to_string();
        if id.starts_with('.') {
            continue;
        }
        let state = load_state(paths, &id).await;
        let dir = entry.path();
        let size_bytes = tokio::task::spawn_blocking(move || dir_size(&dir))
            .await
            .unwrap_or(0);
        out.push(DiskInstance {
            name: if state.name.is_empty() {
                id.clone()
            } else {
                state.name.clone()
            },
            id,
            project: state.project,
            pack_version: state.pack_version,
            installed: state.installed,
            size_bytes,
        });
    }
    out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    out
}

pub async fn delete(paths: &Paths, id: &str) -> Result<()> {
    let dir = paths.instance_dir(id);
    if dir.exists() {
        tokio::fs::remove_dir_all(&dir).await?;
    }
    Ok(())
}

/// Значення options.txt для пресетів графіки. Невідомі ключі Minecraft просто ігнорує,
/// тож набір підходить і для старих, і для нових версій.
pub fn preset_values(preset: &str) -> Vec<(&'static str, &'static str)> {
    match preset {
        "low" => vec![
            ("renderDistance", "6"),
            ("simulationDistance", "6"),
            ("graphicsMode", "0"),
            ("fancyGraphics", "false"),
            ("ao", "false"),
            ("renderClouds", "\"false\""),
            ("particles", "2"),
            ("entityShadows", "false"),
            ("biomeBlendRadius", "0"),
            ("mipmapLevels", "0"),
            ("entityDistanceScaling", "0.5"),
            ("maxFps", "60"),
        ],
        "high" => vec![
            ("renderDistance", "16"),
            ("simulationDistance", "10"),
            ("graphicsMode", "1"),
            ("fancyGraphics", "true"),
            ("ao", "true"),
            ("renderClouds", "\"true\""),
            ("particles", "0"),
            ("entityShadows", "true"),
            ("biomeBlendRadius", "2"),
            ("mipmapLevels", "4"),
            ("entityDistanceScaling", "1.0"),
            ("maxFps", "144"),
        ],
        "ultra" => vec![
            ("renderDistance", "24"),
            ("simulationDistance", "12"),
            ("graphicsMode", "2"),
            ("fancyGraphics", "true"),
            ("ao", "true"),
            ("renderClouds", "\"true\""),
            ("particles", "0"),
            ("entityShadows", "true"),
            ("biomeBlendRadius", "4"),
            ("mipmapLevels", "4"),
            ("entityDistanceScaling", "1.25"),
            ("maxFps", "260"),
        ],
        _ => vec![
            ("renderDistance", "10"),
            ("simulationDistance", "8"),
            ("graphicsMode", "1"),
            ("fancyGraphics", "true"),
            ("ao", "true"),
            ("renderClouds", "\"fast\""),
            ("particles", "1"),
            ("entityShadows", "true"),
            ("biomeBlendRadius", "1"),
            ("mipmapLevels", "2"),
            ("entityDistanceScaling", "1.0"),
            ("maxFps", "120"),
        ],
    }
}

/// Оновлює лише вказані ключі в options.txt, решту (гучність, клавіші, чутливість) не чіпає.
pub fn merge_options(dir: &Path, values: &[(&str, String)]) -> Result<()> {
    let path = dir.join("options.txt");
    let existing = std::fs::read_to_string(&path).unwrap_or_default();
    let mut lines: Vec<String> = existing
        .lines()
        .map(|line| line.trim_end_matches('\r').to_string())
        .filter(|line| !line.is_empty())
        .collect();
    for (key, value) in values {
        let prefix = format!("{key}:");
        let replacement = format!("{key}:{value}");
        match lines.iter_mut().find(|line| line.starts_with(&prefix)) {
            Some(line) => *line = replacement,
            None => lines.push(replacement),
        }
    }
    std::fs::create_dir_all(dir)?;
    let mut text = lines.join("\n");
    text.push('\n');
    std::fs::write(&path, text)?;
    Ok(())
}

pub async fn apply_preset(paths: &Paths, id: &str, preset: &str) -> Result<()> {
    let dir = paths.instance_dir(id);
    let values: Vec<(&str, String)> = preset_values(preset)
        .into_iter()
        .map(|(k, v)| (k, v.to_string()))
        .collect();
    tokio::task::spawn_blocking(move || merge_options(&dir, &values)).await??;
    Ok(())
}

pub async fn apply_lang(paths: &Paths, id: &str, lang: &str) -> Result<()> {
    let dir = paths.instance_dir(id);
    let values = vec![("lang", lang.to_string())];
    tokio::task::spawn_blocking(move || merge_options(&dir, &values)).await??;
    Ok(())
}

pub fn tail_text(path: &Path, max_bytes: u64) -> String {
    use std::io::{Read, Seek, SeekFrom};
    let Ok(mut file) = std::fs::File::open(path) else {
        return String::new();
    };
    let length = file.metadata().map(|m| m.len()).unwrap_or(0);
    if length > max_bytes {
        let _ = file.seek(SeekFrom::Start(length - max_bytes));
    }
    let mut bytes = Vec::new();
    let _ = file.read_to_end(&mut bytes);
    String::from_utf8_lossy(&bytes).to_string()
}

pub fn last_lines(text: &str, count: usize) -> String {
    let lines: Vec<&str> = text.lines().collect();
    let start = lines.len().saturating_sub(count);
    lines[start..].join("\n")
}
