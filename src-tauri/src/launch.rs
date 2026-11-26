use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

use reqwest::Client;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};
use tokio::process::Command;

use crate::constants::LAUNCHER_NAME;
use crate::error::{fail, Context, Result};
use crate::events::{phase, status, step};
use crate::i18n::tr;
use crate::instance;
use crate::java;
use crate::loaders;
use crate::manifest::Manifest;
use crate::minecraft::{self, AssetsInfo};
use crate::paths::Paths;
use crate::t;
use crate::util::hide_window;
use crate::version::{merge, rules_allow, ArgValue, Argument, VersionJson};

#[derive(Clone, Serialize)]
struct GameState {
    state: String,
    instance: String,
    code: Option<i32>,
    crashed: bool,
    hint: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct LastExit {
    pub code: Option<i32>,
    pub crashed: bool,
    pub hint: Option<String>,
    pub ram_mb: u32,
    pub java: String,
}

/// Усе, що потрібно для запуску, вже завантажене й перевірене.
pub struct Prepared {
    pub version: VersionJson,
    pub java: PathBuf,
    pub classpath: Vec<PathBuf>,
    pub client_jar: PathBuf,
    pub natives_dir: PathBuf,
    pub assets: AssetsInfo,
    pub mc: String,
}

pub struct LaunchRequest<'a> {
    pub instance_id: &'a str,
    pub nickname: &'a str,
    pub ram_mb: u32,
}

pub fn offline_uuid(nickname: &str) -> String {
    let mut bytes = md5::compute(format!("OfflinePlayer:{nickname}").as_bytes()).0;
    bytes[6] = (bytes[6] & 0x0f) | 0x30;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    let hex = hex::encode(bytes);
    format!(
        "{}-{}-{}-{}-{}",
        &hex[0..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..32]
    )
}

fn path_str(path: &Path) -> String {
    path.to_string_lossy().to_string()
}

fn substitute(input: &str, vars: &HashMap<&'static str, String>) -> String {
    let mut out = input.to_string();
    for (key, value) in vars {
        out = out.replace(&format!("${{{key}}}"), value);
    }
    out
}

fn expand(argument: &Argument, vars: &HashMap<&'static str, String>) -> Vec<String> {
    match argument {
        Argument::Plain(text) => vec![substitute(text, vars)],
        Argument::Conditional { rules, value } => {
            if !rules_allow(rules) {
                return Vec::new();
            }
            match value {
                ArgValue::One(text) => vec![substitute(text, vars)],
                ArgValue::Many(list) => list.iter().map(|t| substitute(t, vars)).collect(),
            }
        }
    }
}

/// Запуск клієнта.
///
/// Minecraft Usage Guidelines: лаунчер не модифікує оригінальні бібліотеки Mojang —
/// `authlib` та решта завантажуються з libraries.minecraft.net з перевіркою SHA-1 і
/// потрапляють у classpath як є; жодних `-javaagent`, authlib-injector чи патчів класів.
/// Офлайн-вхід працює виключно через стандартні аргументи запуску клієнта:
/// `--username`, `--uuid` (офлайн-UUID з ніка), `--accessToken 0`, `--userType legacy`.
fn spawn_game(
    paths: &Paths,
    game_dir: &Path,
    log_path: &Path,
    a: &Prepared,
    request: &LaunchRequest,
) -> Result<tokio::process::Child> {
    let mut entries: Vec<String> = a.classpath.iter().map(|p| path_str(p)).collect();
    entries.push(path_str(&a.client_jar));
    let classpath_str = entries.join(";");

    let vars: HashMap<&'static str, String> = HashMap::from([
        ("auth_player_name", request.nickname.to_string()),
        ("version_name", a.mc.clone()),
        ("game_directory", path_str(game_dir)),
        ("assets_root", path_str(&paths.assets())),
        ("assets_index_name", a.assets.index_id.clone()),
        ("game_assets", path_str(&a.assets.game_assets)),
        ("auth_uuid", offline_uuid(request.nickname)),
        ("auth_access_token", "0".to_string()),
        ("auth_session", "0".to_string()),
        ("clientid", "0".to_string()),
        ("auth_xuid", "0".to_string()),
        ("user_type", "legacy".to_string()),
        (
            "version_type",
            a.version
                .version_type
                .clone()
                .unwrap_or_else(|| "release".to_string()),
        ),
        ("user_properties", "{}".to_string()),
        ("profile_name", request.nickname.to_string()),
        ("natives_directory", path_str(&a.natives_dir)),
        ("launcher_name", LAUNCHER_NAME.to_string()),
        ("launcher_version", env!("CARGO_PKG_VERSION").to_string()),
        ("classpath", classpath_str.clone()),
        ("classpath_separator", ";".to_string()),
        ("library_directory", path_str(&paths.libraries())),
        ("primary_jar", path_str(&a.client_jar)),
    ]);

    let mut jvm: Vec<String> = vec![
        format!("-Xmx{}M", request.ram_mb),
        format!("-Xms{}M", request.ram_mb.min(1024)),
        "-Dfile.encoding=UTF-8".to_string(),
    ];
    if let Some(arguments) = &a.version.arguments {
        for argument in &arguments.jvm {
            jvm.extend(expand(argument, &vars));
        }
    }
    if !jvm.iter().any(|x| x.starts_with("-Djava.library.path=")) {
        jvm.push(format!("-Djava.library.path={}", path_str(&a.natives_dir)));
    }
    if !jvm.iter().any(|x| x == "-cp" || x == "-classpath") {
        jvm.push("-cp".to_string());
        jvm.push(classpath_str.clone());
    }

    let mut game: Vec<String> = Vec::new();
    if let Some(arguments) = &a.version.arguments {
        for argument in &arguments.game {
            game.extend(expand(argument, &vars));
        }
    }
    if game.is_empty() {
        if let Some(legacy) = &a.version.minecraft_arguments {
            game.extend(
                legacy
                    .split_whitespace()
                    .map(|part| substitute(part, &vars)),
            );
        }
    }

    let main_class = a
        .version
        .main_class
        .clone()
        .ctx("у версії не вказано mainClass")?;

    if let Some(parent) = log_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let log = std::fs::File::create(log_path)?;
    let log_err = log.try_clone()?;

    let mut command = Command::new(&a.java);
    command
        .args(&jvm)
        .arg(&main_class)
        .args(&game)
        .current_dir(game_dir)
        .stdin(Stdio::null())
        .stdout(Stdio::from(log))
        .stderr(Stdio::from(log_err));
    hide_window(&mut command);

    command
        .spawn()
        .ctx(tr("не вдалося запустити Java", "failed to start Java"))
}

/// Шукає у виводі гри відомі причини вильоту, щоб показати гравцю зрозумілу підказку.
pub fn diagnose(text: &str) -> Option<String> {
    let checks: [(&str, &[&str]); 5] = [
        (
            "memory_too_high",
            &[
                "Could not reserve enough space",
                "Invalid maximum heap size",
                "Initial heap size set to a larger value",
                "There is insufficient memory for the Java Runtime",
            ],
        ),
        ("memory_low", &["java.lang.OutOfMemoryError"]),
        (
            "java",
            &[
                "UnsupportedClassVersionError",
                "has been compiled by a more recent version",
            ],
        ),
        (
            "mods",
            &[
                "ModResolutionException",
                "Incompatible mods found",
                "Incompatible mod set",
                "Missing or unsupported mandatory dependencies",
                "DuplicateModsFoundException",
                "Mod Loading has failed",
                "Found a duplicate mod",
                "requires version",
            ],
        ),
        (
            "graphics",
            &[
                "Pixel format not accelerated",
                "GLFW error 65542",
                "does not appear to support OpenGL",
                "Couldn't set pixel format",
            ],
        ),
    ];
    for (hint, needles) in checks {
        if needles.iter().any(|needle| text.contains(needle)) {
            return Some(hint.to_string());
        }
    }
    None
}

fn newest_crash_report(game_dir: &Path, since: SystemTime) -> Option<PathBuf> {
    let entries = std::fs::read_dir(game_dir.join("crash-reports")).ok()?;
    entries
        .flatten()
        .filter_map(|entry| {
            let modified = entry.metadata().ok()?.modified().ok()?;
            (modified >= since).then(|| (modified, entry.path()))
        })
        .max_by_key(|(modified, _)| *modified)
        .map(|(_, path)| path)
}

pub fn latest_crash_report(game_dir: &Path) -> Option<PathBuf> {
    newest_crash_report(game_dir, SystemTime::UNIX_EPOCH)
}

/// Підкладає скін активного профілю для мода CustomSkinLoader (якщо він є у збірці).
/// Без мода файл просто лежить і нічого не ламає.
pub async fn place_skin(
    paths: &Paths,
    game_dir: &Path,
    profile_id: &str,
    nickname: &str,
    has_skin: bool,
) {
    let target_dir = game_dir
        .join("CustomSkinLoader")
        .join("LocalSkin")
        .join("skins");
    let target = target_dir.join(format!("{nickname}.png"));
    if has_skin {
        let source = paths.skin_file(profile_id);
        if source.is_file() && tokio::fs::create_dir_all(&target_dir).await.is_ok() {
            let _ = tokio::fs::copy(&source, &target).await;
        }
    } else if target.is_file() {
        let _ = tokio::fs::remove_file(&target).await;
    }
}

pub async fn prepare(
    app: &AppHandle,
    client: &Client,
    paths: &Paths,
    manifest: &Manifest,
    game_dir: &Path,
    deep: bool,
) -> Result<Prepared> {
    let mc = manifest.minecraft_version.trim().to_string();
    paths.ensure_shared_dirs().await?;
    phase(app, "preparing");

    step(app, "minecraft");
    status(
        app,
        &t!("Перевірка Minecraft {}…", "Checking Minecraft {}…", mc),
    );
    let vanilla = minecraft::load_vanilla(app, client, paths, &mc).await?;
    // Клієнт не залежить від Java, тож качаємо його в межах етапу «Minecraft».
    let client_jar = minecraft::ensure_client_jar(app, client, paths, &vanilla, &mc, deep).await?;

    let java_major = manifest
        .java_major
        .or_else(|| vanilla.java_version.as_ref().map(|j| j.major_version))
        .unwrap_or(8);
    step(app, "java");
    status(
        app,
        &t!("Перевірка Java {}…", "Checking Java {}…", java_major),
    );
    let java_path = java::ensure_java(app, client, paths, java_major).await?;

    step(app, "loader");
    let loader_name = manifest.loader.kind.trim();
    status(
        app,
        &if loader_name.is_empty() || loader_name.eq_ignore_ascii_case("vanilla") {
            tr("Завантажувач модів не потрібен (Vanilla)", "No mod loader needed (Vanilla)").to_string()
        } else {
            t!(
                "Перевірка {} {}…",
                "Checking {} {}…",
                loader_name,
                manifest.loader.version.trim()
            )
        },
    );
    let loader_id = loaders::prepare_loader(app, client, paths, manifest, &java_path).await?;

    let version = if loader_id == mc {
        vanilla
    } else {
        let child = minecraft::read_version(paths, &loader_id).await?;
        merge(vanilla, child)
    };

    let natives_dir = paths.version_dir(&version.id).join("natives");
    step(app, "libraries");
    status(app, tr("Перевірка бібліотек гри…", "Checking game libraries…"));
    let classpath =
        minecraft::install_libraries(app, client, paths, &version, &natives_dir, deep).await?;
    step(app, "assets");
    status(app, tr("Перевірка ресурсів гри…", "Checking game assets…"));
    let assets = minecraft::install_assets(app, client, paths, &version, game_dir, deep).await?;

    Ok(Prepared {
        version,
        java: java_path,
        classpath,
        client_jar,
        natives_dir,
        assets,
        mc,
    })
}

pub fn spawn(
    app: &AppHandle,
    paths: &Paths,
    slot: Arc<Mutex<Option<String>>>,
    prepared: &Prepared,
    request: &LaunchRequest,
) -> Result<()> {
    let game_dir = paths.instance_dir(request.instance_id);
    let log_path = instance::game_log(paths, request.instance_id);

    {
        let mut running = slot
            .lock()
            .map_err(|_| crate::error::AppError("game lock".into()))?;
        if running.is_some() {
            return fail(tr(
                "Гра вже запущена. Закрий її, щоб запустити знову.",
                "The game is already running. Close it to start again.",
            ));
        }
        *running = Some(request.instance_id.to_string());
    }

    step(app, "launch");
    phase(app, "launching");
    status(
        app,
        &t!(
            "Запуск Minecraft {} · {} · ОЗП {} МБ…",
            "Starting Minecraft {} · {} · {} MB RAM…",
            prepared.mc,
            request.nickname,
            request.ram_mb
        ),
    );
    let started = SystemTime::now();
    let spawned = spawn_game(paths, &game_dir, &log_path, prepared, request);
    let mut child = match spawned {
        Ok(child) => child,
        Err(error) => {
            if let Ok(mut running) = slot.lock() {
                *running = None;
            }
            return Err(error);
        }
    };

    let instance_id = request.instance_id.to_string();
    let _ = app.emit(
        "game-state",
        GameState {
            state: "running".to_string(),
            instance: instance_id.clone(),
            code: None,
            crashed: false,
            hint: None,
        },
    );
    status(app, tr("Гра запущена", "Game is running"));

    let handle = app.clone();
    let meta_dir = paths.instance_meta(&instance_id);
    let ram_mb = request.ram_mb;
    let java = prepared.java.to_string_lossy().to_string();
    tauri::async_runtime::spawn(async move {
        let code = match child.wait().await {
            Ok(exit) => exit.code(),
            Err(_) => None,
        };
        let (crashed, hint) = tokio::task::spawn_blocking(move || {
            let report = newest_crash_report(&game_dir, started);
            let failed = matches!(code, Some(c) if c != 0) || report.is_some();
            if !failed {
                return (false, None);
            }
            let mut text = instance::tail_text(&log_path, 256 * 1024);
            if let Some(report) = report {
                text.push_str(&instance::tail_text(&report, 64 * 1024));
            }
            (true, diagnose(&text))
        })
        .await
        .unwrap_or((false, None));

        let last = LastExit {
            code,
            crashed,
            hint: hint.clone(),
            ram_mb,
            java,
        };
        if let Ok(text) = serde_json::to_string_pretty(&last) {
            let _ = tokio::fs::write(meta_dir.join("last_exit.json"), text).await;
        }
        if let Ok(mut running) = slot.lock() {
            *running = None;
        }
        let _ = handle.emit(
            "game-state",
            GameState {
                state: "exited".to_string(),
                instance: instance_id,
                code,
                crashed,
                hint,
            },
        );
    });
    Ok(())
}
