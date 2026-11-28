//! Власні збірки гравця: будь-яка версія Minecraft з серверів Mojang
//! і будь-який завантажувач (Fabric, Forge, NeoForge, Quilt) — без маніфесту проєкту.
//! Сюди ж потрапляють готові збірки спільноти з Modrinth / CurseForge (див. `modpack.rs`).

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
const MAX_ORIGIN: usize = 80;

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
    /// Звідки збірка: «Modrinth · 6.1.0», «CurseForge · 2.4» (порожньо — створена вручну).
    #[serde(default)]
    pub origin: String,
    /// Хто з друзів бачить збірку: `private` | `public` | `on_request` (див. [`VISIBILITIES`]).
    #[serde(default = "default_visibility")]
    pub visibility: String,
    /// id опублікованої копії в `shared_packs` (порожньо — не опубліковано).
    #[serde(default)]
    pub shared_id: String,
    /// Відбиток вмісту на момент публікації: якщо змінився — копія друзям оновлюється.
    #[serde(default)]
    pub shared_fingerprint: String,
    /// Збірку завантажено від друга: id його спільної копії (кнопка «Відкрити збірку»).
    #[serde(default)]
    pub shared_from: String,
}

pub const VISIBILITIES: [&str; 3] = ["private", "public", "on_request"];

fn default_visibility() -> String {
    "private".to_string()
}

impl CustomSpec {
    /// Збірку видно друзям (і її треба тримати опублікованою).
    pub fn is_shared(&self) -> bool {
        matches!(self.visibility.as_str(), "public" | "on_request")
    }

    /// Тип збірки для Rich Presence.
    pub fn presence_type(&self) -> &'static str {
        match self.visibility.as_str() {
            "public" => "custom_public",
            "on_request" => "custom_request",
            _ => "custom_private",
        }
    }

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

// ───────────────────────────── іконка збірки ─────────────────────────────

/// Іконка власної збірки лежить поруч із custom.json: `instances/<id>/.launcher/icon.png`.
/// Гра її не бачить, а «Видалити, залишивши світи» прибирає разом з описом збірки.
pub const ICON_FILE: &str = "icon.png";
/// Інтерфейс сам зменшує картинку до 128×128 PNG (зазвичай 5–40 КБ), тож це з запасом.
pub const MAX_ICON_BYTES: usize = 256 * 1024;
const MAX_ICON_SIDE: u32 = 512;

pub fn icon_file(paths: &Paths, id: &str) -> std::path::PathBuf {
    paths.instance_meta(id).join(ICON_FILE)
}

/// Іконка — справжній PNG розумного розміру (не довільний файл під виглядом картинки).
pub fn check_icon(bytes: &[u8]) -> Result<()> {
    if bytes.len() > MAX_ICON_BYTES {
        return fail(tr("Іконка завелика", "The icon is too large"));
    }
    match crate::commands::png_size(bytes) {
        Some((w, h)) if (16..=MAX_ICON_SIDE).contains(&w) && (16..=MAX_ICON_SIDE).contains(&h) => Ok(()),
        _ => fail(tr(
            "Іконка має бути PNG від 16×16 до 512×512",
            "The icon must be a PNG from 16×16 to 512×512",
        )),
    }
}

/// Записує (`Some`) або прибирає (`None`) іконку власної збірки.
pub async fn save_icon(paths: &Paths, id: &str, bytes: Option<&[u8]>) -> Result<()> {
    let path = icon_file(paths, id);
    match bytes {
        Some(bytes) => {
            check_icon(bytes)?;
            tokio::fs::create_dir_all(paths.instance_meta(id)).await?;
            // Через тимчасовий файл: обірваний запис не лишить «биту» іконку.
            let temp = path.with_extension("png.part");
            tokio::fs::write(&temp, bytes).await?;
            tokio::fs::rename(&temp, &path).await?;
        }
        None => {
            let _ = tokio::fs::remove_file(&path).await;
        }
    }
    Ok(())
}

/// Іконка як data-URL для інтерфейсу (порожньо — іконки немає або файл пошкоджено).
pub async fn icon_data_url(paths: &Paths, id: &str) -> String {
    match tokio::fs::read(icon_file(paths, id)).await {
        Ok(bytes) if check_icon(&bytes).is_ok() => format!("data:image/png;base64,{}", base64(&bytes)),
        _ => String::new(),
    }
}

fn base64(bytes: &[u8]) -> String {
    const ABC: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = (u32::from(chunk[0]) << 16)
            | (u32::from(*chunk.get(1).unwrap_or(&0)) << 8)
            | u32::from(*chunk.get(2).unwrap_or(&0));
        for i in 0..4 {
            if i <= chunk.len() {
                out.push(ABC[(n >> (18 - 6 * i) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
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
    spec.name = spec
        .name
        .trim()
        .chars()
        .filter(|c| !c.is_control())
        .take(MAX_NAME)
        .collect::<String>()
        .trim()
        .to_string();
    if spec.name.is_empty() {
        return fail(tr("Вкажи назву збірки", "Enter a modpack name"));
    }
    spec.origin = spec
        .origin
        .trim()
        .chars()
        .filter(|c| !c.is_control())
        .take(MAX_ORIGIN)
        .collect();
    if !VISIBILITIES.contains(&spec.visibility.as_str()) {
        spec.visibility = default_visibility();
    }
    if !is_uuid(&spec.shared_from) {
        spec.shared_from.clear();
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

/// UUID у канонічному вигляді (id спільних збірок і акаунтів у Supabase).
pub fn is_uuid(value: &str) -> bool {
    value.len() == 36
        && value.char_indices().all(|(i, c)| match i {
            8 | 13 | 18 | 23 => c == '-',
            _ => c.is_ascii_hexdigit(),
        })
}

pub async fn create(paths: &Paths, mut spec: CustomSpec) -> Result<String> {
    validate(&mut spec)?;
    // Публікацію робить окрема команда — нова збірка ще не має спільної копії.
    spec.shared_id.clear();
    spec.shared_fingerprint.clear();
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

#[cfg(test)]
mod tests {
    use super::*;

    fn png(w: u32, h: u32) -> Vec<u8> {
        let mut bytes = vec![0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a, 0, 0, 0, 13];
        bytes.extend_from_slice(b"IHDR");
        bytes.extend_from_slice(&w.to_be_bytes());
        bytes.extend_from_slice(&h.to_be_bytes());
        bytes.extend_from_slice(&[8, 6, 0, 0, 0]);
        bytes
    }

    #[test]
    fn base64_matches_rfc4648() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foo"), "Zm9v");
        assert_eq!(base64(b"foobar"), "Zm9vYmFy");
        assert_eq!(base64(&[0xff, 0xfe, 0xfd, 0x00]), "//79AA==");
    }

    #[test]
    fn icons_must_be_reasonable_pngs() {
        assert!(check_icon(&png(128, 128)).is_ok());
        assert!(check_icon(&png(16, 512)).is_ok());
        assert!(check_icon(&png(8, 8)).is_err());
        assert!(check_icon(&png(4096, 4096)).is_err());
        assert!(check_icon(b"GIF89a not a png at all......").is_err());
        let mut huge = png(128, 128);
        huge.resize(MAX_ICON_BYTES + 1, 0);
        assert!(check_icon(&huge).is_err());
    }
}
