use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use futures_util::{stream, StreamExt};
use reqwest::Client;
use serde::Serialize;
use tauri::AppHandle;

use crate::error::{AppError, Result};
use crate::events::{phase, status, step, Progress};
use crate::i18n::tr;
use crate::instance::{self, PROTECTED};
use crate::manifest::{
    is_config_like, is_world_junk, is_world_path, safe_relative, Manifest, ManifestFile,
};
use crate::paths::Paths;
use crate::t;
use crate::util::{download_file, verify_file_async, Checksum};

#[derive(Serialize, Clone, Debug, Default)]
pub struct SyncReport {
    pub checked: usize,
    pub downloaded: usize,
    pub removed: usize,
}

struct Entry {
    file: ManifestFile,
    dest: PathBuf,
    recorded: Option<String>,
}

enum Decision {
    Keep,
    Download,
    /// Конфіг, який гравець змінив сам, або вже наявний файл світу з `saves/` —
    /// не чіпаємо, лише запам'ятовуємо еталон.
    KeepUserCopy,
}

fn is_protected(root: &Path, path: &Path) -> bool {
    let Ok(relative) = path.strip_prefix(root) else {
        return true;
    };
    let Some(first) = relative.components().next() else {
        return true;
    };
    let first = first.as_os_str().to_string_lossy();
    PROTECTED.iter().any(|p| p.eq_ignore_ascii_case(&first))
}

/// Видаляє з mods/ лише ті .jar, яких немає в еталонній збірці.
fn remove_unlisted(root: &Path, keep: &HashSet<String>) -> Result<usize> {
    let mods = root.join("mods");
    let mut removed = 0usize;
    if !mods.is_dir() {
        return Ok(0);
    }
    let mut stack = vec![mods];
    while let Some(current) = stack.pop() {
        for item in std::fs::read_dir(&current)? {
            let item = item?;
            let path = item.path();
            if is_protected(root, &path) {
                continue;
            }
            if item.file_type()?.is_dir() {
                stack.push(path);
                continue;
            }
            let is_jar = path
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| e.eq_ignore_ascii_case("jar"))
                .unwrap_or(false);
            if is_jar && !keep.contains(&path.to_string_lossy().to_lowercase()) {
                std::fs::remove_file(&path)?;
                removed += 1;
            }
        }
    }
    Ok(removed)
}

async fn decide(entry: &Entry, deep: bool) -> Decision {
    let expected = entry.file.sha256.to_lowercase();
    let exists = entry.dest.is_file();

    // Стартовий світ: докачуємо лише відсутні файли. Якщо файл уже є — це прогрес гравця,
    // його не перезаписуємо навіть у режимі «Ремонт» (і зайві файли світу ніколи не видаляються).
    if is_world_path(&entry.file.path) {
        return if exists || is_world_junk(&entry.file.path) {
            Decision::KeepUserCopy
        } else {
            Decision::Download
        };
    }

    if is_config_like(&entry.file.path) {
        if !exists {
            return Decision::Download;
        }
        return match &entry.recorded {
            // Адмін змінив файл з моменту минулого встановлення — оновлюємо.
            Some(recorded) if *recorded != expected => Decision::Download,
            Some(_) => Decision::Keep,
            None => {
                let same = verify_file_async(entry.dest.clone(), Checksum::Sha256(expected)).await;
                if same {
                    Decision::Keep
                } else {
                    Decision::KeepUserCopy
                }
            }
        };
    }

    if !exists {
        return Decision::Download;
    }
    if !deep {
        let size_ok = entry.file.size == 0
            || std::fs::metadata(&entry.dest)
                .map(|m| m.len() == entry.file.size)
                .unwrap_or(false);
        if size_ok && entry.recorded.as_deref() == Some(expected.as_str()) {
            return Decision::Keep;
        }
    }
    if verify_file_async(entry.dest.clone(), Checksum::Sha256(expected)).await {
        Decision::Keep
    } else {
        Decision::Download
    }
}

/// Синхронізує файли збірки строго в межах `instances/<id>/`.
/// `deep = true` — режим "Ремонт": SHA-256 кожного файлу без швидких перевірок.
/// `prune = true` — одразу видалити зайві .jar з mods/. Під час запуску гри цим займається
/// дебагер збірки (з детальним звітом і карантином), тож там `prune = false`.
pub async fn sync_instance(
    app: &AppHandle,
    client: &Client,
    paths: &Paths,
    id: &str,
    manifest: &Manifest,
    deep: bool,
    prune: bool,
) -> Result<SyncReport> {
    instance::ensure_dirs(paths, id).await?;
    let root = paths.instance_dir(id);
    let mut state = instance::load_state(paths, id).await;

    let mut entries = Vec::with_capacity(manifest.files.len());
    for file in &manifest.files {
        let relative = safe_relative(&file.path)?;
        entries.push(Entry {
            recorded: state.files.get(&file.path).map(|s| s.to_lowercase()),
            file: file.clone(),
            dest: root.join(relative),
        });
    }
    let keep: HashSet<String> = entries
        .iter()
        .map(|e| e.dest.to_string_lossy().to_lowercase())
        .collect();

    step(app, "files");
    status(
        app,
        &t!(
            "Перевірка файлів збірки: {}…",
            "Checking modpack files: {}…",
            entries.len()
        ),
    );
    phase(app, "checking");
    let verify = Arc::new(Progress::new(app, "verify", entries.len() as u64));
    let checked: Vec<(Entry, Decision)> = stream::iter(entries)
        .map(|entry| {
            let verify = verify.clone();
            async move {
                verify.set_file(&entry.file.path);
                let decision = decide(&entry, deep).await;
                verify.add(1);
                (entry, decision)
            }
        })
        .buffer_unordered(8)
        .collect()
        .await;
    verify.finish();

    let mut report = SyncReport {
        checked: checked.len(),
        ..SyncReport::default()
    };
    let listed: HashSet<String> = manifest.files.iter().map(|f| f.path.clone()).collect();
    state.files.retain(|path, _| listed.contains(path));

    let mut todo = Vec::new();
    for (entry, decision) in checked {
        match decision {
            Decision::Keep | Decision::KeepUserCopy => {
                state
                    .files
                    .insert(entry.file.path.clone(), entry.file.sha256.to_lowercase());
            }
            Decision::Download => todo.push(entry),
        }
    }

    let mut first_error: Option<AppError> = None;
    if todo.is_empty() {
        status(
            app,
            tr("Усі файли збірки актуальні", "All modpack files are up to date"),
        );
    } else {
        let bytes: u64 = todo.iter().map(|e| e.file.size).sum();
        status(
            app,
            &t!(
                "Потрібно завантажити файлів: {} ({} МБ)",
                "{} files to download ({} MB)",
                todo.len(),
                (bytes + 1024 * 1024 - 1) / (1024 * 1024)
            ),
        );
    }
    if !todo.is_empty() {
        phase(app, "downloading");
        let total: u64 = todo.iter().map(|e| e.file.size).sum();
        status(
            app,
            &t!(
                "Завантаження файлів збірки: {}…",
                "Downloading modpack files: {}…",
                todo.len()
            ),
        );
        let progress = Arc::new(Progress::new(app, "download", total));
        let results: Vec<(String, String, Result<()>)> = stream::iter(todo)
            .map(|entry| {
                let client = client.clone();
                let progress = progress.clone();
                async move {
                    progress.set_file(&entry.file.path);
                    let checksum = Checksum::Sha256(entry.file.sha256.clone());
                    let reporter = progress.clone();
                    let result = download_file(
                        &client,
                        &entry.file.url,
                        &entry.dest,
                        &checksum,
                        &move |n| reporter.add(n),
                    )
                    .await;
                    (entry.file.path, entry.file.sha256.to_lowercase(), result)
                }
            })
            .buffer_unordered(4)
            .collect()
            .await;
        progress.finish();
        for (path, sha, result) in results {
            match result {
                Ok(()) => {
                    report.downloaded += 1;
                    state.files.insert(path, sha);
                }
                Err(error) => {
                    if first_error.is_none() {
                        first_error = Some(error);
                    }
                }
            }
        }
    }

    if first_error.is_none() {
        if prune {
            let keep_root = root.clone();
            report.removed =
                tokio::task::spawn_blocking(move || remove_unlisted(&keep_root, &keep)).await??;
        }
        if report.removed > 0 {
            status(
                app,
                &t!(
                    "Видалено зайвих модів: {}",
                    "Removed extra mods: {}",
                    report.removed
                ),
            );
        }
        state.installed = true;
        state.kept_personal = false;
        state.pack_version = manifest.version.clone();
    }
    instance::save_state(paths, id, &state).await?;

    match first_error {
        Some(error) => Err(error),
        None => Ok(report),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha256};

    fn temp_root(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "terrapolis-sync-test-{name}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn entry(root: &Path, path: &str, content: &[u8], recorded: Option<&str>) -> Entry {
        Entry {
            file: ManifestFile {
                path: path.to_string(),
                sha256: hex::encode(Sha256::digest(content)),
                size: content.len() as u64,
                url: format!("https://example.invalid/{path}"),
            },
            dest: root.join(safe_relative(path).unwrap()),
            recorded: recorded.map(str::to_string),
        }
    }

    #[tokio::test]
    async fn world_files_are_downloaded_only_when_missing() {
        let root = temp_root("saves");
        let admin = b"level from the modpack";

        // Файлу ще немає — стартовий світ докачується.
        let missing = entry(&root, "saves/HW25/level.dat", admin, None);
        assert!(matches!(decide(&missing, false).await, Decision::Download));
        assert!(matches!(decide(&missing, true).await, Decision::Download));

        // Гравець уже грав у світі: файл відрізняється від еталона, але його не чіпаємо —
        // ні під час звичайної перевірки, ні в режимі «Ремонт», ні після оновлення збірки.
        let player = root.join("saves").join("HW25").join("level.dat");
        std::fs::create_dir_all(player.parent().unwrap()).unwrap();
        std::fs::write(&player, b"player progress").unwrap();
        let changed_by_admin = entry(&root, "saves/HW25/level.dat", admin, Some("0".repeat(64).as_str()));
        for deep in [false, true] {
            assert!(matches!(decide(&changed_by_admin, deep).await, Decision::KeepUserCopy));
        }
        assert_eq!(std::fs::read(&player).unwrap(), b"player progress");

        // Службові файли світу ніколи не завантажуються.
        let lock = entry(&root, "saves/HW25/session.lock", b"lock", None);
        assert!(matches!(decide(&lock, true).await, Decision::KeepUserCopy));

        // Звичайні файли збірки й далі оновлюються, якщо відрізняються.
        let jar = root.join("mods").join("a.jar");
        std::fs::create_dir_all(jar.parent().unwrap()).unwrap();
        std::fs::write(&jar, b"old jar").unwrap();
        let mod_entry = entry(&root, "mods/a.jar", b"new jar", None);
        assert!(matches!(decide(&mod_entry, false).await, Decision::Download));

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn prune_never_touches_saves() {
        let root = temp_root("prune");
        let world = root.join("saves").join("w").join("region").join("r.0.0.mca");
        std::fs::create_dir_all(world.parent().unwrap()).unwrap();
        std::fs::write(&world, b"region").unwrap();
        let extra = root.join("mods").join("extra.jar");
        std::fs::create_dir_all(extra.parent().unwrap()).unwrap();
        std::fs::write(&extra, b"jar").unwrap();

        let removed = remove_unlisted(&root, &HashSet::new()).unwrap();
        assert_eq!(removed, 1);
        assert!(world.is_file());
        assert!(!extra.exists());
        let _ = std::fs::remove_dir_all(&root);
    }
}
