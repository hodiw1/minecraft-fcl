//! Імпорт збірки з `.zip`: коли всі джерела недоступні, адміністратор може роздати архів,
//! а гравець — встановити його без інтернету.
//!
//! Розпаковуються ЛИШЕ `manifest.json`, `mods/`, `config/`, `resourcepacks/`, `shaderpacks/`,
//! `saves/` та `servers.dat`. Файли стартових світів із `saves/` лише докладаються, коли їх ще
//! немає: наявні світи гравця не перезаписуються. Скріншоти, `options.txt` та інші особисті
//! файли гравця не чіпаються, навіть якщо вони є в архіві.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::error::{fail, AppError, Context, Result};
use crate::i18n::tr;
use crate::manifest::{self, safe_relative, Manifest};
use crate::t;

const MAX_ENTRY_BYTES: u64 = 4 * 1024 * 1024 * 1024;
const MAX_TOTAL_BYTES: u64 = 24 * 1024 * 1024 * 1024;

#[derive(Serialize, Clone, Debug, Default)]
pub struct ImportReport {
    pub instance_id: String,
    pub name: String,
    /// Створено нову власну збірку з архіву.
    pub created: bool,
    pub pack_version: String,
    /// Розпаковано файлів.
    pub files: usize,
    /// Файлів, SHA-256 яких збігся з manifest.json.
    pub verified: usize,
    /// Файли з архіву, що відрізняються від manifest.json (буде виправлено онлайн).
    pub mismatched: Vec<String>,
    /// Записів архіву, пропущених як особисті або сторонні (saves/, options.txt…).
    pub skipped: usize,
}

/// Що знайдено в архіві до розпакування.
pub struct Inspected {
    /// Префікс кореня збірки всередині архіву ("" або "MyPack/").
    pub prefix: String,
    pub manifest: Option<Manifest>,
}

fn normalize_base(base: &str) -> String {
    let base = base.trim().replace('\\', "/");
    let base = base.trim_matches('/');
    if base.is_empty() {
        String::new()
    } else {
        format!("{base}/")
    }
}

fn open(zip_path: &Path) -> Result<zip::ZipArchive<std::fs::File>> {
    let file = std::fs::File::open(zip_path).ctx(tr(
        "Не вдалося відкрити архів",
        "Couldn't open the archive",
    ))?;
    zip::ZipArchive::new(file).ctx(tr(
        "Файл не схожий на .zip архів збірки",
        "This file doesn't look like a modpack .zip archive",
    ))
}

fn depth(name: &str) -> usize {
    name.matches('/').count()
}

/// Знаходить корінь збірки в архіві: папку з manifest.json (найменш вкладену) або з mods/.
pub fn inspect(zip_path: &Path, base: &str) -> Result<Inspected> {
    let mut archive = open(zip_path)?;
    let names: Vec<String> = archive.file_names().map(|n| n.replace('\\', "/")).collect();

    let base = normalize_base(base);
    let prefix = if !base.is_empty() {
        if !names.iter().any(|n| n.starts_with(&base)) {
            return fail(t!(
                "У архіві немає папки «{}»",
                "The archive has no folder \"{}\"",
                base.trim_end_matches('/')
            ));
        }
        base
    } else if let Some(found) = names
        .iter()
        .filter(|n| *n == "manifest.json" || n.ends_with("/manifest.json"))
        .min_by_key(|n| depth(n))
    {
        found.trim_end_matches("manifest.json").to_string()
    } else if let Some(found) = names
        .iter()
        .filter_map(|n| {
            if n.starts_with("mods/") {
                Some(String::new())
            } else {
                n.find("/mods/").map(|i| n[..=i].to_string())
            }
        })
        .min_by_key(|p| depth(p))
    {
        found
    } else {
        return fail(tr(
            "В архіві не знайдено ні manifest.json, ні папки mods/ — це не схоже на збірку",
            "Neither manifest.json nor a mods/ folder was found — this doesn't look like a modpack",
        ));
    };

    let manifest = match archive.by_name(&format!("{prefix}manifest.json")) {
        Ok(entry) => {
            let mut text = String::new();
            entry
                .take(4 * 1024 * 1024)
                .read_to_string(&mut text)
                .ctx("manifest.json")?;
            Some(manifest::parse(&text).map_err(|error| {
                AppError(format!(
                    "{}: {}",
                    tr(
                        "manifest.json в архіві пошкоджений",
                        "manifest.json in the archive is invalid",
                    ),
                    error.0
                ))
            })?)
        }
        Err(_) => None,
    };
    Ok(Inspected { prefix, manifest })
}

fn part_of(dest: &Path) -> PathBuf {
    let mut name = dest.as_os_str().to_os_string();
    name.push(".part");
    PathBuf::from(name)
}

/// Розпаковує дозволені частини збірки в `instance_dir`. Повертає (шлях, sha256) кожного файлу.
pub fn extract(
    zip_path: &Path,
    prefix: &str,
    instance_dir: &Path,
    report: &mut ImportReport,
) -> Result<Vec<(String, String)>> {
    let mut archive = open(zip_path)?;
    let mut written: Vec<(String, String)> = Vec::new();
    let mut total: u64 = 0;
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index)?;
        if entry.is_dir() {
            continue;
        }
        let name = entry.name().replace('\\', "/");
        let Some(relative) = name.strip_prefix(prefix) else {
            report.skipped += 1;
            continue;
        };
        // manifest.json читаємо окремо; решта — лише дозволені папки (safe_relative відсікає
        // options.txt, screenshots/, "..", абсолютні шляхи тощо).
        if relative == "manifest.json" {
            continue;
        }
        let Ok(safe) = safe_relative(relative) else {
            report.skipped += 1;
            continue;
        };
        if entry.size() > MAX_ENTRY_BYTES {
            return fail(t!("Файл в архіві завеликий: {}", "File in the archive is too large: {}", relative));
        }
        total += entry.size();
        if total > MAX_TOTAL_BYTES {
            return fail(tr("Архів завеликий", "The archive is too large"));
        }

        let dest = instance_dir.join(&safe);
        // Світ гравця не перезаписуємо: з архіву беремо лише відсутні файли, без службових.
        if manifest::is_world_path(relative) && (dest.exists() || manifest::is_world_junk(relative)) {
            report.skipped += 1;
            continue;
        }
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let temp = part_of(&dest);
        let mut out = std::fs::File::create(&temp)?;
        let mut hasher = Sha256::new();
        let mut buffer = vec![0u8; 65536];
        let mut copied: u64 = 0;
        loop {
            let read = entry.read(&mut buffer)?;
            if read == 0 {
                break;
            }
            copied += read as u64;
            if copied > MAX_ENTRY_BYTES {
                drop(out);
                let _ = std::fs::remove_file(&temp);
                return fail(tr("Архів пошкоджений або завеликий", "The archive is damaged or too large"));
            }
            hasher.update(&buffer[..read]);
            out.write_all(&buffer[..read])?;
        }
        out.flush()?;
        drop(out);
        std::fs::rename(&temp, &dest)?;
        written.push((relative.to_string(), hex::encode(hasher.finalize())));
    }
    report.files = written.len();
    Ok(written)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_base() {
        assert_eq!(normalize_base(""), "");
        assert_eq!(normalize_base("\\MyPack\\"), "MyPack/");
        assert_eq!(normalize_base("a/b"), "a/b/");
    }

    #[test]
    fn personal_files_are_never_extracted() {
        for path in ["options.txt", "screenshots/a.png", "../x.jar", "C:/x.jar", "saves/../x.jar"] {
            assert!(safe_relative(path).is_err(), "{path}");
        }
        for path in ["mods/a.jar", "config/a.toml", "resourcepacks/r.zip", "shaderpacks/s.zip", "servers.dat", "saves/world/level.dat"] {
            assert!(safe_relative(path).is_ok(), "{path}");
        }
    }
}
