use std::path::{Component, Path, PathBuf};

use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::error::{fail, Result};
use crate::mirrors;

/// Кореневі папки всередині `pack/`, які може містити збірка. `saves/` — стартові світи:
/// їх лаунчер лише докачує, коли файлу ще немає, і ніколи не перезаписує (див. [`is_world_path`]).
pub const MANAGED_DIRS: [&str; 5] = ["mods", "config", "resourcepacks", "shaderpacks", "saves"];

/// Службові файли світу, які не мають потрапляти в збірку: блокування сесії
/// й резервна копія level.dat, яку гра перезаписує сама.
pub const WORLD_JUNK: [&str; 2] = ["session.lock", "level.dat_old"];

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct LoaderSpec {
    #[serde(rename = "type", default)]
    pub kind: String,
    #[serde(default)]
    pub version: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ManifestFile {
    pub path: String,
    pub sha256: String,
    #[serde(default)]
    pub size: u64,
    pub url: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Manifest {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub version: String,
    pub minecraft_version: String,
    #[serde(default)]
    pub loader: LoaderSpec,
    #[serde(default)]
    pub java_major: Option<u32>,
    #[serde(default)]
    pub recommended_ram_mb: Option<u32>,
    #[serde(default)]
    pub files: Vec<ManifestFile>,
}

pub fn is_safe_token(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_' | '+'))
}

pub fn safe_relative(path: &str) -> Result<PathBuf> {
    let mut out = PathBuf::new();
    for component in Path::new(path).components() {
        match component {
            Component::Normal(part) => out.push(part),
            _ => return fail(format!("небезпечний шлях у маніфесті: {path}")),
        }
    }
    if path == "servers.dat" {
        return Ok(out);
    }
    let mut parts = out.components();
    let first = parts
        .next()
        .and_then(|c| c.as_os_str().to_str().map(|s| s.to_string()));
    let has_name = parts.next().is_some();
    match first {
        Some(dir) if has_name && MANAGED_DIRS.contains(&dir.as_str()) => Ok(out),
        _ => fail(format!(
            "шлях поза дозволеними папками (mods, config, resourcepacks, shaderpacks, saves): {path}"
        )),
    }
}

/// Файл стартового світу (`saves/<світ>/...`): прогрес гравця, який лаунчер не перезаписує.
pub fn is_world_path(path: &str) -> bool {
    path.starts_with("saves/")
}

/// `session.lock` / `level.dat_old` усередині `saves/` — службові файли, їх не публікуємо й не розпаковуємо.
pub fn is_world_junk(path: &str) -> bool {
    is_world_path(path)
        && path
            .rsplit('/')
            .next()
            .map(|name| WORLD_JUNK.iter().any(|junk| junk.eq_ignore_ascii_case(name)))
            .unwrap_or(false)
}

fn validate(manifest: &Manifest) -> Result<()> {
    if !is_safe_token(manifest.minecraft_version.trim()) {
        return fail("некоректна minecraft_version у маніфесті");
    }
    let loader_version = manifest.loader.version.trim();
    if !loader_version.is_empty() && !is_safe_token(loader_version) {
        return fail("некоректна версія завантажувача у маніфесті");
    }
    for file in &manifest.files {
        safe_relative(&file.path)?;
        if !file.url.starts_with("https://") {
            return fail(format!("посилання не https: {}", file.path));
        }
        if file.sha256.len() != 64 || !file.sha256.chars().all(|c| c.is_ascii_hexdigit()) {
            return fail(format!("некоректний sha256 для {}", file.path));
        }
    }
    Ok(())
}

pub fn is_config_like(path: &str) -> bool {
    path == "servers.dat" || path.starts_with("config/")
}

pub fn total_size(manifest: &Manifest) -> u64 {
    manifest.files.iter().map(|f| f.size).sum()
}

/// manifest.json збірки: оригінальне посилання, а за потреби — дзеркала (без кешу CDN).
pub async fn fetch(client: &Client, url: &str) -> Result<Manifest> {
    let manifest: Manifest = mirrors::get_json(client, url, true)
        .await
        .map_err(|error| error.into_plain(url))?;
    validate(&manifest)?;
    Ok(manifest)
}

/// Розбір manifest.json з архіву чи диска з тими самими перевірками, що й для мережі.
pub fn parse(text: &str) -> Result<Manifest> {
    let manifest: Manifest = serde_json::from_str(text.trim_start_matches('\u{feff}'))?;
    validate(&manifest)?;
    Ok(manifest)
}

/// Чи є файл модом у mods/ (саме .jar, будь-якої вкладеності).
pub fn is_mod_path(path: &str) -> bool {
    path.starts_with("mods/") && path.to_ascii_lowercase().ends_with(".jar")
}

pub async fn load_cached(path: &Path) -> Option<Manifest> {
    let text = tokio::fs::read_to_string(path).await.ok()?;
    let manifest: Manifest = serde_json::from_str(&text).ok()?;
    validate(&manifest).ok()?;
    Some(manifest)
}

pub async fn save_cache(path: &Path, manifest: &Manifest) -> Result<()> {
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    let text = serde_json::to_string_pretty(manifest)?;
    tokio::fs::write(path, text).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saves_are_allowed_but_traversal_is_not() {
        for path in [
            "saves/HW25/level.dat",
            "saves/WINTER DebtHunt v0.5/region/r.0.0.mca",
            "mods/a.jar",
            "servers.dat",
        ] {
            assert!(safe_relative(path).is_ok(), "{path}");
        }
        for path in [
            "saves",
            "saves/../mods/x.jar",
            "saves/../../evil.exe",
            "../saves/w/level.dat",
            "/saves/w/level.dat",
            "C:/saves/w/level.dat",
            "options.txt",
            "screenshots/a.png",
        ] {
            assert!(safe_relative(path).is_err(), "{path}");
        }
    }

    #[test]
    fn world_junk_is_detected() {
        assert!(is_world_path("saves/w/level.dat"));
        assert!(!is_world_path("config/saves/x.cfg"));
        assert!(is_world_junk("saves/w/session.lock"));
        assert!(is_world_junk("saves/w/level.dat_old"));
        assert!(!is_world_junk("saves/w/level.dat"));
        assert!(!is_world_junk("config/session.lock"));
    }
}
