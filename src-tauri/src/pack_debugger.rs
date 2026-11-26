//! Pre-launch дебагер цілісності збірки проєкту.
//!
//! Працює лише для збірок прив'язаного проєкту (є manifest.json проєкту); власні збірки
//! гравця не чіпає. Звіряє `instances/<id>/mods/` зі списком модів у маніфесті:
//! * відсутні моди — докачує (з дзеркалами);
//! * пошкоджені (SHA-256 не збігається) — завантажує заново поверх битого файлу;
//! * зайві `.jar`, яких немає в маніфесті (додані гравцем випадково або від старої версії
//!   збірки), — прибирає з mods/ у карантин `.launcher/removed-mods/`, звідки їх можна повернути.
//!
//! SHA-256 кешується за (розмір, час зміни), тож повторна перевірка перед кожним запуском
//! займає частку секунди.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::UNIX_EPOCH;

use futures_util::{stream, StreamExt};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tauri::AppHandle;

use crate::error::Result;
use crate::events::{status, step, Progress};
use crate::i18n::{is_en, tr};
use crate::instance;
use crate::manifest::{is_mod_path, safe_relative, Manifest, ManifestFile};
use crate::paths::Paths;
use crate::t;
use crate::util::{download_file, sha256_file, Checksum};

/// Куди переносяться зайві моди (відносно папки збірки).
pub const QUARANTINE: &str = ".launcher/removed-mods";
const CACHE_FILE: &str = "hash-cache.json";

#[derive(Serialize, Clone, Debug, Default)]
pub struct DebugReport {
    /// Скільки модів перелічено в маніфесті.
    pub checked: usize,
    /// Скільки з них на місці й з правильним SHA-256.
    pub ok: usize,
    pub missing: Vec<String>,
    pub corrupt: Vec<String>,
    pub extra: Vec<String>,
    /// Відсутні моди, які вдалося докачати.
    pub downloaded: Vec<String>,
    /// Пошкоджені моди, замінені свіжою копією.
    pub replaced: Vec<String>,
    /// Зайві моди, прибрані з mods/ (у карантин).
    pub removed: Vec<String>,
    /// Що виправити не вдалося (немає мережі, файл зайнятий тощо).
    pub failed: Vec<String>,
    /// Перевірка без мережі: лише звіт і карантин, без завантажень.
    pub offline: bool,
    pub quarantine: String,
    pub pack_version: String,
    /// Готовий підсумок для панелі прогресу («видалено 2 зайві моди, докачано 1 відсутній мод»).
    pub summary: String,
}

impl DebugReport {
    pub fn is_clean(&self) -> bool {
        self.missing.is_empty() && self.corrupt.is_empty() && self.extra.is_empty()
    }
}

#[derive(Serialize, Deserialize, Clone, Default)]
struct CachedHash {
    size: u64,
    modified: u64,
    sha256: String,
}

type HashCache = HashMap<String, CachedHash>;

#[derive(Clone, Copy, PartialEq)]
enum Health {
    Ok,
    Missing,
    Corrupt,
}

fn plural(n: usize, uk: [&str; 3], en: [&str; 2]) -> String {
    if is_en() {
        return format!("{n} {}", if n == 1 { en[0] } else { en[1] });
    }
    let (m10, m100) = (n % 10, n % 100);
    let form = if m10 == 1 && m100 != 11 {
        0
    } else if (2..=4).contains(&m10) && !(12..=14).contains(&m100) {
        1
    } else {
        2
    };
    format!("{n} {}", uk[form])
}

fn display_name(path: &str) -> String {
    path.strip_prefix("mods/").unwrap_or(path).to_string()
}

fn modified_micros(meta: &std::fs::Metadata) -> u64 {
    meta.modified()
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_micros() as u64)
        .unwrap_or(0)
}

fn cache_path(paths: &Paths, id: &str) -> PathBuf {
    paths.instance_meta(id).join(CACHE_FILE)
}

async fn load_cache(paths: &Paths, id: &str) -> HashCache {
    match tokio::fs::read_to_string(cache_path(paths, id)).await {
        Ok(text) => serde_json::from_str(&text).unwrap_or_default(),
        Err(_) => HashCache::new(),
    }
}

async fn save_cache(paths: &Paths, id: &str, cache: &HashCache) {
    let path = cache_path(paths, id);
    if let Some(parent) = path.parent() {
        let _ = tokio::fs::create_dir_all(parent).await;
    }
    if let Ok(text) = serde_json::to_string(cache) {
        let _ = tokio::fs::write(path, text).await;
    }
}

/// Усі .jar у mods/ (з підпапками) → (відносний шлях через "/", повний шлях).
fn scan_jars(root: &Path) -> Vec<(String, PathBuf)> {
    let mut out = Vec::new();
    let mut stack = vec![root.join("mods")];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            if kind.is_dir() {
                // Службові папки завантажувачів (.connector, memory_repo) — не моди гравця.
                let name = entry.file_name().to_string_lossy().to_lowercase();
                if !name.starts_with('.') && name != "memory_repo" {
                    stack.push(path);
                }
                continue;
            }
            let is_jar = path
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| e.eq_ignore_ascii_case("jar"))
                .unwrap_or(false);
            if !kind.is_file() || !is_jar {
                continue;
            }
            let Ok(relative) = path.strip_prefix(root) else {
                continue;
            };
            let relative = relative
                .components()
                .map(|c| c.as_os_str().to_string_lossy().to_string())
                .collect::<Vec<_>>()
                .join("/");
            out.push((relative, path));
        }
    }
    out.sort();
    out
}

/// Стан одного мода з маніфесту; SHA-256 береться з кешу, якщо файл не змінювався.
fn check_file(path: &Path, file: &ManifestFile, cache: &Mutex<HashCache>) -> Health {
    let Ok(meta) = std::fs::metadata(path) else {
        return Health::Missing;
    };
    if !meta.is_file() {
        return Health::Missing;
    }
    let size = meta.len();
    if file.size > 0 && size != file.size {
        return Health::Corrupt;
    }
    let modified = modified_micros(&meta);
    let key = file.path.to_lowercase();
    let cached = cache.lock().ok().and_then(|c| c.get(&key).cloned());
    let sha = match cached {
        Some(entry) if entry.size == size && entry.modified == modified && modified != 0 => entry.sha256,
        _ => match sha256_file(path) {
            Ok(hash) => {
                if let Ok(mut c) = cache.lock() {
                    c.insert(
                        key,
                        CachedHash {
                            size,
                            modified,
                            sha256: hash.clone(),
                        },
                    );
                }
                hash
            }
            Err(_) => return Health::Corrupt,
        },
    };
    if sha.eq_ignore_ascii_case(&file.sha256) {
        Health::Ok
    } else {
        Health::Corrupt
    }
}

/// Переносить зайвий мод у карантин (якщо не вийшло — видаляє).
fn quarantine(meta_dir: &Path, relative: &str, path: &Path) -> std::io::Result<()> {
    let target_dir = meta_dir.join("removed-mods");
    std::fs::create_dir_all(&target_dir)?;
    let name = relative.trim_start_matches("mods/").replace('/', "_");
    let target = target_dir.join(name);
    if target.exists() {
        let _ = std::fs::remove_file(&target);
    }
    match std::fs::rename(path, &target) {
        Ok(()) => Ok(()),
        Err(_) => std::fs::remove_file(path),
    }
}

fn summary(report: &DebugReport) -> String {
    let lead = tr("Дебагер збірки", "Pack debugger");
    if report.is_clean() {
        return t!(
            "{}: перевірено {} — проблем не знайдено",
            "{}: checked {} — no problems found",
            lead,
            plural(report.checked, ["мод", "моди", "модів"], ["mod", "mods"])
        );
    }
    let mut parts: Vec<String> = Vec::new();
    if !report.removed.is_empty() {
        parts.push(t!(
            "видалено {}",
            "removed {}",
            plural(
                report.removed.len(),
                ["зайвий мод", "зайві моди", "зайвих модів"],
                ["extra mod", "extra mods"]
            )
        ));
    }
    if !report.downloaded.is_empty() {
        parts.push(t!(
            "докачано {}",
            "downloaded {}",
            plural(
                report.downloaded.len(),
                ["відсутній мод", "відсутні моди", "відсутніх модів"],
                ["missing mod", "missing mods"]
            )
        ));
    }
    if !report.replaced.is_empty() {
        parts.push(t!(
            "замінено {}",
            "replaced {}",
            plural(
                report.replaced.len(),
                ["пошкоджений мод", "пошкоджені моди", "пошкоджених модів"],
                ["damaged mod", "damaged mods"]
            )
        ));
    }
    if !report.failed.is_empty() {
        let what = plural(report.failed.len(), ["мод", "моди", "модів"], ["mod", "mods"]);
        parts.push(if report.offline {
            t!(
                "не виправлено {} (немає мережі)",
                "{} not fixed (offline)",
                what
            )
        } else {
            t!("не вдалося виправити {}", "couldn't fix {}", what)
        });
    }
    format!("{lead}: {}", parts.join(", "))
}

/// Перевіряє й виправляє mods/ збірки за маніфестом. `offline = true` — без завантажень:
/// зайві моди все одно прибираються, а відсутні/пошкоджені лише потрапляють у звіт.
pub async fn run(
    app: &AppHandle,
    client: &Client,
    paths: &Paths,
    id: &str,
    manifest: &Manifest,
    offline: bool,
) -> Result<DebugReport> {
    step(app, "debug");
    status(
        app,
        tr(
            "Дебагер збірки: звіряю mods/ з manifest.json…",
            "Pack debugger: comparing mods/ with manifest.json…",
        ),
    );
    let root = paths.instance_dir(id);
    let meta_dir = paths.instance_meta(id);
    tokio::fs::create_dir_all(root.join("mods")).await?;

    let mut expected: Vec<(ManifestFile, PathBuf)> = Vec::new();
    for file in manifest.files.iter().filter(|f| is_mod_path(&f.path)) {
        if let Ok(relative) = safe_relative(&file.path) {
            expected.push((file.clone(), root.join(relative)));
        }
    }
    let listed: HashSet<String> = expected.iter().map(|(f, _)| f.path.to_lowercase()).collect();

    let mut report = DebugReport {
        checked: expected.len(),
        offline,
        quarantine: QUARANTINE.to_string(),
        pack_version: manifest.version.clone(),
        ..DebugReport::default()
    };

    // 1. Зайві .jar.
    let scan_root = root.clone();
    let on_disk = tokio::task::spawn_blocking(move || scan_jars(&scan_root)).await?;
    let extras: Vec<(String, PathBuf)> = on_disk
        .into_iter()
        .filter(|(relative, _)| !listed.contains(&relative.to_lowercase()))
        .collect();

    // 2. Наявність і SHA-256 модів з маніфесту.
    let cache = Arc::new(Mutex::new(load_cache(paths, id).await));
    let progress = Arc::new(Progress::new(app, "debug", expected.len() as u64));
    let checked: Vec<(ManifestFile, PathBuf, Health)> = stream::iter(expected)
        .map(|(file, path)| {
            let cache = cache.clone();
            let progress = progress.clone();
            async move {
                progress.set_file(&display_name(&file.path));
                let probe_path = path.clone();
                let probe_file = file.clone();
                let health =
                    tokio::task::spawn_blocking(move || check_file(&probe_path, &probe_file, &cache))
                        .await
                        .unwrap_or(Health::Corrupt);
                progress.add(1);
                (file, path, health)
            }
        })
        .buffer_unordered(4)
        .collect()
        .await;
    progress.finish();

    // 3. Зайві — у карантин.
    for (relative, path) in &extras {
        let name = display_name(relative);
        report.extra.push(name.clone());
        let (meta, relative, path) = (meta_dir.clone(), relative.clone(), path.clone());
        let moved = tokio::task::spawn_blocking(move || quarantine(&meta, &relative, &path))
            .await
            .map(|r| r.is_ok())
            .unwrap_or(false);
        if moved {
            report.removed.push(name);
        } else {
            report.failed.push(name);
        }
    }

    // 4. Відсутні й пошкоджені — завантажити.
    let mut todo: Vec<(ManifestFile, PathBuf, Health)> = Vec::new();
    for (file, path, health) in checked {
        match health {
            Health::Ok => report.ok += 1,
            Health::Missing => {
                report.missing.push(display_name(&file.path));
                todo.push((file, path, health));
            }
            Health::Corrupt => {
                report.corrupt.push(display_name(&file.path));
                todo.push((file, path, health));
            }
        }
    }

    let mut fixed: Vec<(String, String)> = Vec::new();
    if !todo.is_empty() {
        if offline {
            report
                .failed
                .extend(todo.iter().map(|(file, _, _)| display_name(&file.path)));
        } else {
            status(
                app,
                &t!(
                    "Дебагер збірки: докачування {}…",
                    "Pack debugger: downloading {}…",
                    plural(todo.len(), ["мода", "модів", "модів"], ["mod", "mods"])
                ),
            );
            let total: u64 = todo.iter().map(|(file, _, _)| file.size).sum();
            let bytes = Arc::new(Progress::new(app, "debug-download", total.max(1)));
            let results: Vec<(ManifestFile, Health, Result<()>)> = stream::iter(todo)
                .map(|(file, path, health)| {
                    let client = client.clone();
                    let bytes = bytes.clone();
                    async move {
                        bytes.set_file(&display_name(&file.path));
                        let reporter = bytes.clone();
                        // Пишемо поверх битого файлу лише після успішної перевірки SHA-256.
                        let result = download_file(
                            &client,
                            &file.url,
                            &path,
                            &Checksum::Sha256(file.sha256.clone()),
                            &move |n| reporter.add(n),
                        )
                        .await;
                        (file, health, result)
                    }
                })
                .buffer_unordered(4)
                .collect()
                .await;
            bytes.finish();
            for (file, health, result) in results {
                let name = display_name(&file.path);
                match result {
                    Ok(()) => {
                        fixed.push((file.path.clone(), file.sha256.to_lowercase()));
                        if health == Health::Missing {
                            report.downloaded.push(name);
                        } else {
                            report.replaced.push(name);
                        }
                    }
                    Err(_) => report.failed.push(name),
                }
            }
        }
    }

    // 5. Кеш хешів і стан збірки.
    let mut cache = cache.lock().map(|c| c.clone()).unwrap_or_default();
    cache.retain(|key, _| key.starts_with("mods/"));
    for (path, sha) in &fixed {
        if let Ok(relative) = safe_relative(path) {
            if let Ok(meta) = std::fs::metadata(root.join(relative)) {
                cache.insert(
                    path.to_lowercase(),
                    CachedHash {
                        size: meta.len(),
                        modified: modified_micros(&meta),
                        sha256: sha.clone(),
                    },
                );
            }
        }
    }
    save_cache(paths, id, &cache).await;
    if !fixed.is_empty() {
        let mut state = instance::load_state(paths, id).await;
        for (path, sha) in fixed {
            state.files.insert(path, sha);
        }
        instance::save_state(paths, id, &state).await?;
    }

    for list in [
        &mut report.missing,
        &mut report.corrupt,
        &mut report.extra,
        &mut report.downloaded,
        &mut report.replaced,
        &mut report.removed,
        &mut report.failed,
    ] {
        list.sort_by_key(|name| name.to_lowercase());
    }
    report.summary = summary(&report);
    status(app, &report.summary);
    Ok(report)
}
