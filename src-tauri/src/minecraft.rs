use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use futures_util::{stream, StreamExt};
use reqwest::Client;
use serde::Deserialize;
use tauri::AppHandle;

use crate::error::{fail, AppError, Context, Result};
use crate::events::{status, Progress};
use crate::i18n::tr;
use crate::paths::Paths;
use crate::t;
use crate::util::{
    download_file, extract_zip, fetch_json, join_rel, noop, verify_file_async, Checksum,
};
use crate::version::{maven_path, rules_allow, Artifact, AssetIndexFile, Library, VersionJson};

const VERSION_MANIFEST_URL: &str =
    "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";

#[derive(Deserialize)]
struct VersionList {
    versions: Vec<VersionEntry>,
}

#[derive(Deserialize)]
struct VersionEntry {
    id: String,
    url: String,
    #[serde(default)]
    sha1: Option<String>,
}

#[derive(Clone)]
struct ResolvedLib {
    path: PathBuf,
    url: String,
    sha1: Option<String>,
    size: Option<u64>,
    native: bool,
    exclude: Vec<String>,
}

pub struct AssetsInfo {
    pub index_id: String,
    pub game_assets: PathBuf,
}

pub async fn read_version(paths: &Paths, id: &str) -> Result<VersionJson> {
    let path = paths.version_json(id);
    let text = tokio::fs::read_to_string(&path)
        .await
        .ctx(&format!("не вдалося прочитати {}", path.display()))?;
    serde_json::from_str::<VersionJson>(&text).ctx("пошкоджений JSON версії")
}

pub async fn load_vanilla(
    app: &AppHandle,
    client: &Client,
    paths: &Paths,
    mc: &str,
) -> Result<VersionJson> {
    if let Ok(version) = read_version(paths, mc).await {
        return Ok(version);
    }
    status(
        app,
        &t!(
            "Завантаження метаданих Minecraft {}…",
            "Downloading Minecraft {} metadata…",
            mc
        ),
    );
    let list: VersionList = fetch_json(client, VERSION_MANIFEST_URL).await?;
    let entry = list
        .versions
        .into_iter()
        .find(|v| v.id == mc)
        .ctx(&format!("версію Minecraft {mc} не знайдено"))?;
    let checksum = match &entry.sha1 {
        Some(hash) => Checksum::Sha1(hash.clone()),
        None => Checksum::None,
    };
    let json_path = paths.version_json(mc);
    download_file(client, &entry.url, &json_path, &checksum, &noop).await?;
    read_version(paths, mc).await
}

pub async fn ensure_client_jar(
    app: &AppHandle,
    client: &Client,
    paths: &Paths,
    version: &VersionJson,
    mc: &str,
    deep: bool,
) -> Result<PathBuf> {
    let jar = paths.version_jar(mc);
    let download = version.downloads.as_ref().and_then(|d| d.client.as_ref());
    match download {
        Some(download) => {
            let present = if deep {
                verify_file_async(jar.clone(), Checksum::Sha1(download.sha1.clone())).await
            } else {
                std::fs::metadata(&jar)
                    .map(|m| m.is_file() && (download.size == 0 || m.len() == download.size))
                    .unwrap_or(false)
            };
            if !present {
                status(
                    app,
                    &t!(
                        "Завантаження клієнта Minecraft {}…",
                        "Downloading Minecraft {} client…",
                        mc
                    ),
                );
                let progress = Arc::new(Progress::new(app, "client", download.size));
                progress.set_file(&format!("{mc}.jar"));
                let reporter = progress.clone();
                download_file(
                    client,
                    &download.url,
                    &jar,
                    &Checksum::Sha1(download.sha1.clone()),
                    &move |n| reporter.add(n),
                )
                .await?;
                progress.finish();
            }
        }
        None => {
            if !jar.is_file() {
                return fail(format!("відсутній клієнтський jar для {mc}"));
            }
        }
    }
    Ok(jar)
}

fn from_artifact(
    base: &Path,
    artifact: &Artifact,
    fallback_name: &str,
    native: bool,
    exclude: Vec<String>,
) -> Option<ResolvedLib> {
    let relative = artifact
        .path
        .clone()
        .or_else(|| maven_path(fallback_name))?;
    Some(ResolvedLib {
        path: join_rel(base, &relative),
        url: artifact.url.clone(),
        sha1: artifact.sha1.clone(),
        size: if artifact.size > 0 {
            Some(artifact.size)
        } else {
            None
        },
        native,
        exclude,
    })
}

fn resolve_libraries(paths: &Paths, libraries: &[Library]) -> Vec<ResolvedLib> {
    let base = paths.libraries();
    let mut out = Vec::new();
    for library in libraries {
        if !rules_allow(&library.rules) {
            continue;
        }
        if library.name.ends_with(":natives-windows-x86") && !cfg!(target_arch = "x86") {
            continue;
        }
        if library.name.ends_with(":natives-windows-arm64") && !cfg!(target_arch = "aarch64") {
            continue;
        }
        let mut handled = false;

        if let Some(template) = library.natives.get("windows") {
            let bits = if cfg!(target_pointer_width = "64") {
                "64"
            } else {
                "32"
            };
            let classifier = template.replace("${arch}", bits);
            let exclude = library
                .extract
                .as_ref()
                .map(|e| e.exclude.clone())
                .unwrap_or_default();
            let artifact = library
                .downloads
                .as_ref()
                .and_then(|d| d.classifiers.get(&classifier));
            if let Some(artifact) = artifact {
                if let Some(resolved) = from_artifact(&base, artifact, &library.name, true, exclude)
                {
                    out.push(resolved);
                    handled = true;
                }
            }
        }

        let main = library.downloads.as_ref().and_then(|d| d.artifact.as_ref());
        if let Some(artifact) = main {
            if let Some(resolved) = from_artifact(&base, artifact, &library.name, false, Vec::new())
            {
                out.push(resolved);
                handled = true;
            }
        }

        if !handled && library.natives.is_empty() {
            if let Some(relative) = maven_path(&library.name) {
                let root = library
                    .url
                    .clone()
                    .unwrap_or_else(|| "https://libraries.minecraft.net/".to_string());
                let root = if root.ends_with('/') {
                    root
                } else {
                    format!("{root}/")
                };
                out.push(ResolvedLib {
                    path: join_rel(&base, &relative),
                    url: format!("{root}{relative}"),
                    sha1: library.sha1.clone(),
                    size: None,
                    native: false,
                    exclude: Vec::new(),
                });
            }
        }
    }
    out
}

fn is_present(library: &ResolvedLib) -> bool {
    match std::fs::metadata(&library.path) {
        Ok(meta) if meta.is_file() => library
            .size
            .map(|size| size == 0 || meta.len() == size)
            .unwrap_or(true),
        _ => false,
    }
}

pub async fn install_libraries(
    app: &AppHandle,
    client: &Client,
    paths: &Paths,
    version: &VersionJson,
    natives_dir: &Path,
    deep: bool,
) -> Result<Vec<PathBuf>> {
    let resolved = resolve_libraries(paths, &version.libraries);
    let mut missing: Vec<ResolvedLib> = Vec::new();
    for library in &resolved {
        let ok = if deep {
            match &library.sha1 {
                Some(hash) => {
                    verify_file_async(library.path.clone(), Checksum::Sha1(hash.clone())).await
                }
                None => is_present(library),
            }
        } else {
            is_present(library)
        };
        if !ok {
            missing.push(library.clone());
        }
    }

    if !missing.is_empty() {
        status(
            app,
            &t!(
                "Завантаження бібліотек: {}…",
                "Downloading libraries: {}…",
                missing.len()
            ),
        );
        let progress = Arc::new(Progress::new(app, "libraries", missing.len() as u64));
        let results: Vec<Result<()>> = stream::iter(missing)
            .map(|library| {
                let client = client.clone();
                let progress = progress.clone();
                async move {
                    let name = library
                        .path
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_default();
                    progress.set_file(&name);
                    if library.url.is_empty() {
                        return fail(format!(
                            "відсутня бібліотека без посилання: {}",
                            library.path.display()
                        ));
                    }
                    let checksum = match &library.sha1 {
                        Some(hash) => Checksum::Sha1(hash.clone()),
                        None => Checksum::None,
                    };
                    download_file(&client, &library.url, &library.path, &checksum, &noop).await?;
                    progress.add(1);
                    Ok::<(), AppError>(())
                }
            })
            .buffer_unordered(8)
            .collect()
            .await;
        for result in results {
            result?;
        }
        progress.finish();
    }

    let marker = natives_dir.join(".extracted");
    if deep || !marker.is_file() {
        tokio::fs::create_dir_all(natives_dir).await?;
        let natives: Vec<ResolvedLib> = resolved.iter().filter(|l| l.native).cloned().collect();
        let target = natives_dir.to_path_buf();
        tokio::task::spawn_blocking(move || -> Result<()> {
            for library in &natives {
                let mut exclude = library.exclude.clone();
                exclude.push("META-INF/".to_string());
                extract_zip(&library.path, &target, &exclude)?;
            }
            std::fs::write(target.join(".extracted"), b"ok")?;
            Ok(())
        })
        .await??;
    }

    let mut seen = HashSet::new();
    let classpath: Vec<PathBuf> = resolved
        .iter()
        .filter(|library| !library.native)
        .filter(|library| seen.insert(library.path.clone()))
        .map(|library| library.path.clone())
        .collect();
    Ok(classpath)
}

pub async fn install_assets(
    app: &AppHandle,
    client: &Client,
    paths: &Paths,
    version: &VersionJson,
    game_dir: &Path,
    deep: bool,
) -> Result<AssetsInfo> {
    let index_ref = version
        .asset_index
        .as_ref()
        .ctx("у версії відсутній assetIndex")?;
    let assets_dir = paths.assets();
    let index_path = assets_dir
        .join("indexes")
        .join(format!("{}.json", index_ref.id));
    let index_checksum = Checksum::Sha1(index_ref.sha1.clone());
    if !verify_file_async(index_path.clone(), index_checksum.clone()).await {
        status(
            app,
            tr("Завантаження індексу ресурсів…", "Downloading asset index…"),
        );
        download_file(client, &index_ref.url, &index_path, &index_checksum, &noop).await?;
    }

    let text = tokio::fs::read_to_string(&index_path).await?;
    let index: AssetIndexFile = serde_json::from_str(&text).ctx("пошкоджений індекс ресурсів")?;
    let objects_dir = assets_dir.join("objects");

    let objects: Vec<(String, u64)> = index
        .objects
        .values()
        .map(|object| (object.hash.clone(), object.size))
        .collect();
    let scan_dir = objects_dir.clone();
    let missing: Vec<(String, PathBuf, String)> = tokio::task::spawn_blocking(move || {
        let mut missing = Vec::new();
        for (hash, size) in objects {
            let Some(prefix) = hash.get(..2) else {
                continue;
            };
            let path = scan_dir.join(prefix).join(&hash);
            let present = std::fs::metadata(&path)
                .map(|m| m.is_file() && (size == 0 || m.len() == size))
                .unwrap_or(false);
            let present = if present && deep {
                crate::util::verify_file(&path, &Checksum::Sha1(hash.clone()))
            } else {
                present
            };
            if !present {
                let url = format!("https://resources.download.minecraft.net/{prefix}/{hash}");
                missing.push((url, path, hash));
            }
        }
        missing
    })
    .await?;

    if !missing.is_empty() {
        status(
            app,
            &t!(
                "Завантаження ресурсів: {}…",
                "Downloading game assets: {}…",
                missing.len()
            ),
        );
        let progress = Arc::new(Progress::new(app, "assets", missing.len() as u64));
        let results: Vec<Result<()>> = stream::iter(missing)
            .map(|(url, path, hash)| {
                let client = client.clone();
                let progress = progress.clone();
                async move {
                    download_file(&client, &url, &path, &Checksum::Sha1(hash), &noop).await?;
                    progress.add(1);
                    Ok::<(), AppError>(())
                }
            })
            .buffer_unordered(16)
            .collect()
            .await;
        for result in results {
            result?;
        }
        progress.finish();
    }

    let mut game_assets = assets_dir.clone();
    if index.is_virtual || index.map_to_resources {
        let target = if index.map_to_resources {
            game_dir.join("resources")
        } else {
            assets_dir.join("virtual").join(&index_ref.id)
        };
        game_assets = target.clone();
        let entries: Vec<(String, PathBuf)> = index
            .objects
            .iter()
            .filter_map(|(name, object)| {
                object
                    .hash
                    .get(..2)
                    .map(|prefix| (name.clone(), objects_dir.join(prefix).join(&object.hash)))
            })
            .collect();
        tokio::task::spawn_blocking(move || -> Result<()> {
            for (name, source) in entries {
                let destination = join_rel(&target, &name);
                if destination.is_file() {
                    continue;
                }
                if let Some(parent) = destination.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                std::fs::copy(&source, &destination)?;
            }
            Ok(())
        })
        .await??;
    }

    Ok(AssetsInfo {
        index_id: index_ref.id.clone(),
        game_assets,
    })
}
