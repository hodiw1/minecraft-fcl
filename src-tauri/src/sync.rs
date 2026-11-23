use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use futures_util::{stream, StreamExt};
use reqwest::Client;
use tauri::AppHandle;

use crate::error::{AppError, Result};
use crate::events::{status, Progress};
use crate::manifest::{safe_relative, Manifest, ManifestFile};
use crate::paths::Paths;
use crate::util::{download_file, verify_file_async, Checksum};

struct Entry {
    file: ManifestFile,
    dest: PathBuf,
}

fn remove_unlisted(dir: &Path, keep: &HashSet<String>) -> Result<usize> {
    let mut removed = 0usize;
    if !dir.is_dir() {
        return Ok(0);
    }
    let mut stack = vec![dir.to_path_buf()];
    while let Some(current) = stack.pop() {
        for item in std::fs::read_dir(&current)? {
            let item = item?;
            let path = item.path();
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

pub async fn sync_pack(
    app: &AppHandle,
    client: &Client,
    paths: &Paths,
    manifest: &Manifest,
) -> Result<()> {
    let mut entries = Vec::with_capacity(manifest.files.len());
    for file in &manifest.files {
        let relative = safe_relative(&file.path)?;
        entries.push(Entry {
            file: file.clone(),
            dest: paths.mc.join(relative),
        });
    }
    let keep: HashSet<String> = entries
        .iter()
        .map(|e| e.dest.to_string_lossy().to_lowercase())
        .collect();

    status(app, "Перевірка SHA-256 файлів збірки…");
    let verify = Arc::new(Progress::new(app, "verify", entries.len() as u64));
    let checked: Vec<(Entry, bool)> = stream::iter(entries)
        .map(|entry| {
            let verify = verify.clone();
            async move {
                verify.set_file(&entry.file.path);
                let ok = entry.dest.is_file()
                    && verify_file_async(
                        entry.dest.clone(),
                        Checksum::Sha256(entry.file.sha256.clone()),
                    )
                    .await;
                verify.add(1);
                (entry, ok)
            }
        })
        .buffer_unordered(8)
        .collect()
        .await;
    verify.finish();

    let todo: Vec<Entry> = checked
        .into_iter()
        .filter(|(_, ok)| !*ok)
        .map(|(entry, _)| entry)
        .collect();

    if todo.is_empty() {
        status(app, "Збірка актуальна");
    } else {
        let total: u64 = todo.iter().map(|e| e.file.size).sum();
        status(app, &format!("Завантаження файлів збірки: {}…", todo.len()));
        let progress = Arc::new(Progress::new(app, "download", total));
        let results: Vec<Result<()>> = stream::iter(todo)
            .map(|entry| {
                let client = client.clone();
                let progress = progress.clone();
                async move {
                    progress.set_file(&entry.file.path);
                    let checksum = Checksum::Sha256(entry.file.sha256.clone());
                    let reporter = progress.clone();
                    download_file(
                        &client,
                        &entry.file.url,
                        &entry.dest,
                        &checksum,
                        &move |n| reporter.add(n),
                    )
                    .await?;
                    Ok::<(), AppError>(())
                }
            })
            .buffer_unordered(4)
            .collect()
            .await;
        for result in results {
            result?;
        }
        progress.finish();
    }

    let mods_dir = paths.mc.join("mods");
    let removed = tokio::task::spawn_blocking(move || remove_unlisted(&mods_dir, &keep)).await??;
    if removed > 0 {
        status(app, &format!("Видалено зайвих модів: {removed}"));
    }
    Ok(())
}
