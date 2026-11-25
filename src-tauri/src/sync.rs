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
use crate::manifest::{is_config_like, safe_relative, Manifest, ManifestFile};
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
    /// Конфіг, який гравець змінив сам — не чіпаємо, лише запам'ятовуємо еталон.
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
pub async fn sync_instance(
    app: &AppHandle,
    client: &Client,
    paths: &Paths,
    id: &str,
    manifest: &Manifest,
    deep: bool,
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
        let keep_root = root.clone();
        report.removed =
            tokio::task::spawn_blocking(move || remove_unlisted(&keep_root, &keep)).await??;
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
