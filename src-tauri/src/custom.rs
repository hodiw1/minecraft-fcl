//! Власні збірки гравця: будь-яка версія Minecraft з серверів Mojang
//! і будь-який завантажувач (Fabric, Forge, NeoForge) — без маніфесту проєкту.

use std::time::{SystemTime, UNIX_EPOCH};

use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::config::new_id;
use crate::error::{fail, Result};
use crate::i18n::tr;
use crate::loaders::LoaderKind;
use crate::manifest::{is_safe_token, LoaderSpec, Manifest};
use crate::paths::Paths;
use crate::util::fetch_json;

const VERSION_MANIFEST_URL: &str =
    "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";
const MAX_NAME: usize = 40;

/// Опис власної збірки: `instances/<id>/.launcher/custom.json`.
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct CustomSpec {
    pub name: String,
    pub minecraft_version: String,
    #[serde(default)]
    pub loader: String,
    #[serde(default)]
    pub loader_version: String,
    #[serde(default)]
    pub created: u64,
}

impl CustomSpec {
    /// Синтетичний маніфест без файлів — далі працює звичайний шлях запуску.
    pub fn to_manifest(&self) -> Manifest {
        Manifest {
            name: self.name.clone(),
            version: String::new(),
            minecraft_version: self.minecraft_version.clone(),
            loader: LoaderSpec {
                kind: self.loader.clone(),
                version: self.loader_version.clone(),
            },
            java_major: None,
            recommended_ram_mb: None,
            files: Vec::new(),
        }
    }
}

fn spec_file(paths: &Paths, id: &str) -> std::path::PathBuf {
    paths.instance_meta(id).join("custom.json")
}

pub async fn load(paths: &Paths, id: &str) -> Option<CustomSpec> {
    let text = tokio::fs::read_to_string(spec_file(paths, id)).await.ok()?;
    serde_json::from_str(&text).ok()
}

pub async fn save(paths: &Paths, id: &str, spec: &CustomSpec) -> Result<()> {
    tokio::fs::create_dir_all(paths.instance_meta(id)).await?;
    tokio::fs::write(spec_file(paths, id), serde_json::to_string_pretty(spec)?).await?;
    Ok(())
}

/// Усі власні збірки, найновіші — вгорі.
pub async fn list(paths: &Paths) -> Vec<(String, CustomSpec)> {
    let mut out = Vec::new();
    let Ok(mut entries) = tokio::fs::read_dir(&paths.instances).await else {
        return out;
    };
    while let Ok(Some(entry)) = entries.next_entry().await {
        let id = entry.file_name().to_string_lossy().to_string();
        if let Some(spec) = load(paths, &id).await {
            out.push((id, spec));
        }
    }
    out.sort_by(|a, b| b.1.created.cmp(&a.1.created));
    out
}

fn validate(spec: &mut CustomSpec) -> Result<()> {
    spec.name = spec.name.trim().chars().take(MAX_NAME).collect();
    if spec.name.is_empty() {
        return fail(tr("Вкажи назву збірки", "Enter a modpack name"));
    }
    spec.minecraft_version = spec.minecraft_version.trim().to_string();
    if !is_safe_token(&spec.minecraft_version) {
        return fail(tr("Обери версію Minecraft", "Choose a Minecraft version"));
    }
    let kind = LoaderKind::parse(&spec.loader)?;
    spec.loader = kind.id().to_string();
    spec.loader_version = spec.loader_version.trim().to_string();
    if kind == LoaderKind::Vanilla {
        spec.loader_version.clear();
    } else if !is_safe_token(&spec.loader_version) {
        return fail(tr(
            "Обери версію завантажувача",
            "Choose a loader version",
        ));
    }
    Ok(())
}

pub async fn create(paths: &Paths, mut spec: CustomSpec) -> Result<String> {
    validate(&mut spec)?;
    spec.created = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let id = format!("my-{}", &new_id()[1..]);
    save(paths, &id, &spec).await?;
    Ok(id)
}

// ───────────────────────────── каталоги версій ─────────────────────────────

#[derive(Serialize)]
pub struct McVersion {
    pub id: String,
    pub kind: String,
}

#[derive(Deserialize)]
struct MojangList {
    versions: Vec<MojangEntry>,
}

#[derive(Deserialize)]
struct MojangEntry {
    id: String,
    #[serde(rename = "type")]
    kind: String,
}

/// Версії Minecraft з офіційного списку Mojang (від найновішої).
pub async fn minecraft_versions(client: &Client, snapshots: bool) -> Result<Vec<McVersion>> {
    let list: MojangList = fetch_json(client, VERSION_MANIFEST_URL).await?;
    Ok(list
        .versions
        .into_iter()
        .filter(|v| snapshots || v.kind == "release")
        .map(|v| McVersion {
            id: v.id,
            kind: v.kind,
        })
        .collect())
}

#[derive(Deserialize)]
struct FabricEntry {
    loader: FabricLoader,
}

#[derive(Deserialize)]
struct FabricLoader {
    version: String,
    /// Fabric позначає стабільні збірки; у Quilt цього поля немає.
    #[serde(default)]
    stable: Option<bool>,
}

#[derive(Deserialize, Default)]
struct ForgePromotions {
    #[serde(default)]
    promos: std::collections::HashMap<String, String>,
}

/// Версія завантажувача для стилізованого списку: бейджі «Recommended» / «Latest».
#[derive(Serialize, Clone, Debug)]
pub struct LoaderVersion {
    pub id: String,
    /// Стабільна (не beta/alpha/pre).
    pub stable: bool,
    /// Рекомендована розробниками завантажувача (або перша стабільна).
    pub recommended: bool,
}

fn looks_unstable(version: &str) -> bool {
    let lower = version.to_lowercase();
    ["beta", "alpha", "pre", "rc", "snapshot"]
        .iter()
        .any(|marker| lower.contains(marker))
}

#[derive(Deserialize)]
struct NeoVersions {
    versions: Vec<String>,
}

async fn fetch_text(client: &Client, url: &str) -> Result<String> {
    Ok(client.get(url).send().await?.error_for_status()?.text().await?)
}

fn xml_versions(xml: &str) -> Vec<String> {
    xml.split("<version>")
        .skip(1)
        .filter_map(|part| part.split("</version>").next())
        .map(|v| v.trim().to_string())
        .collect()
}

/// Префікс версій NeoForge для версії Minecraft: 1.21.1 → "21.1.", 1.21 → "21.0.", 26.1 → "26.1.".
fn neoforge_prefix(mc: &str) -> String {
    match mc.strip_prefix("1.") {
        Some(rest) => {
            let mut parts = rest.split('.');
            let major = parts.next().unwrap_or("");
            let minor = parts.next().unwrap_or("0");
            format!("{major}.{minor}.")
        }
        None => format!("{mc}."),
    }
}

/// Версії завантажувача для конкретної версії Minecraft (від найновішої) з позначками
/// стабільності й рекомендованої версії.
pub async fn loader_versions(client: &Client, loader: &str, mc: &str) -> Result<Vec<LoaderVersion>> {
    let mc = mc.trim();
    if !is_safe_token(mc) {
        return fail(tr("Обери версію Minecraft", "Choose a Minecraft version"));
    }
    let kind = LoaderKind::parse(loader)?;
    // Стабільні версії за даними meta-сервера (Fabric) і рекомендована (Forge promotions).
    let mut stable_flags: std::collections::HashMap<String, bool> = std::collections::HashMap::new();
    let mut promoted: Option<String> = None;
    let mut out: Vec<String> = match kind {
        LoaderKind::Vanilla => Vec::new(),
        LoaderKind::Fabric | LoaderKind::Quilt => {
            let url = if kind == LoaderKind::Quilt {
                format!("https://meta.quiltmc.org/v3/versions/loader/{mc}")
            } else {
                format!("https://meta.fabricmc.net/v2/versions/loader/{mc}")
            };
            let list: Vec<FabricEntry> = fetch_json(client, &url).await?;
            list.into_iter()
                .map(|e| {
                    if let Some(stable) = e.loader.stable {
                        stable_flags.insert(e.loader.version.clone(), stable);
                    }
                    e.loader.version
                })
                .collect()
        }
        LoaderKind::Forge => {
            let promos_url =
                "https://files.minecraftforge.net/net/minecraftforge/forge/promotions_slim.json";
            if let Ok(promotions) = fetch_json::<ForgePromotions>(client, promos_url).await {
                promoted = promotions
                    .promos
                    .get(&format!("{mc}-recommended"))
                    .or_else(|| promotions.promos.get(&format!("{mc}-latest")))
                    .cloned();
            }
            let xml = fetch_text(
                client,
                "https://maven.minecraftforge.net/net/minecraftforge/forge/maven-metadata.xml",
            )
            .await?;
            let prefix = format!("{mc}-");
            let mut list: Vec<String> = xml_versions(&xml)
                .into_iter()
                .filter_map(|v| v.strip_prefix(&prefix).map(str::to_string))
                .collect();
            list.reverse();
            list
        }
        LoaderKind::NeoForge => {
            if mc == "1.20.1" {
                let url = "https://maven.neoforged.net/api/maven/versions/releases/net/neoforged/forge";
                let list: NeoVersions = fetch_json(client, url).await?;
                let prefix = format!("{mc}-");
                let mut list: Vec<String> = list
                    .versions
                    .into_iter()
                    .filter_map(|v| v.strip_prefix(&prefix).map(str::to_string))
                    .collect();
                list.reverse();
                list
            } else {
                let url =
                    "https://maven.neoforged.net/api/maven/versions/releases/net/neoforged/neoforge";
                let list: NeoVersions = fetch_json(client, url).await?;
                let prefix = neoforge_prefix(mc);
                let mut list: Vec<String> = list
                    .versions
                    .into_iter()
                    .filter(|v| v.starts_with(&prefix))
                    .collect();
                list.reverse();
                list
            }
        }
    };
    out.retain(|v| is_safe_token(v));
    // Списки на Maven не завжди впорядковані — сортуємо самі, від найновішої.
    out.sort_by(|a, b| version_key(b).cmp(&version_key(a)));
    out.dedup();

    let stable = |v: &str| {
        stable_flags
            .get(v)
            .copied()
            .unwrap_or_else(|| !looks_unstable(v))
    };
    let recommended = promoted
        .filter(|p| out.contains(p))
        .or_else(|| out.iter().find(|v| stable(v)).cloned());
    Ok(out
        .into_iter()
        .map(|id| LoaderVersion {
            stable: stable(&id),
            recommended: recommended.as_deref() == Some(id.as_str()),
            id,
        })
        .collect())
}

/// Числові частини версії для порівняння: "47.4.29" → [47, 4, 29].
fn version_key(version: &str) -> Vec<u64> {
    version
        .split(|c: char| !c.is_ascii_digit())
        .filter(|part| !part.is_empty())
        .map(|part| part.parse().unwrap_or(0))
        .collect()
}
