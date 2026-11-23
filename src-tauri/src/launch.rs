use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;

use reqwest::Client;
use serde::Serialize;
use tauri::{AppHandle, Emitter};
use tokio::process::Command;

use crate::constants::LAUNCHER_NAME;
use crate::error::{Context, Result};
use crate::events::status;
use crate::java;
use crate::loaders;
use crate::manifest::Manifest;
use crate::minecraft::{self, AssetsInfo};
use crate::paths::Paths;
use crate::util::hide_window;
use crate::version::{merge, rules_allow, ArgValue, Argument, VersionJson};

#[derive(Clone, Serialize)]
struct GameState {
    state: String,
    code: Option<i32>,
}

struct SpawnArgs<'a> {
    version: &'a VersionJson,
    java: &'a Path,
    classpath: &'a [PathBuf],
    client_jar: &'a Path,
    natives_dir: &'a Path,
    assets: &'a AssetsInfo,
    nickname: &'a str,
    ram_mb: u32,
    mc: &'a str,
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

fn spawn_game(app: &AppHandle, paths: &Paths, a: &SpawnArgs) -> Result<()> {
    let mut entries: Vec<String> = a.classpath.iter().map(|p| path_str(p)).collect();
    entries.push(path_str(a.client_jar));
    let classpath_str = entries.join(";");

    let vars: HashMap<&'static str, String> = HashMap::from([
        ("auth_player_name", a.nickname.to_string()),
        ("version_name", a.mc.to_string()),
        ("game_directory", path_str(&paths.mc)),
        ("assets_root", path_str(&paths.assets())),
        ("assets_index_name", a.assets.index_id.clone()),
        ("game_assets", path_str(&a.assets.game_assets)),
        ("auth_uuid", offline_uuid(a.nickname)),
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
        ("profile_name", a.nickname.to_string()),
        ("natives_directory", path_str(a.natives_dir)),
        ("launcher_name", LAUNCHER_NAME.to_string()),
        ("launcher_version", env!("CARGO_PKG_VERSION").to_string()),
        ("classpath", classpath_str.clone()),
        ("classpath_separator", ";".to_string()),
        ("library_directory", path_str(&paths.libraries())),
        ("primary_jar", path_str(a.client_jar)),
    ]);

    let mut jvm: Vec<String> = vec![
        format!("-Xmx{}M", a.ram_mb),
        format!("-Xms{}M", a.ram_mb.min(1024)),
        "-Dfile.encoding=UTF-8".to_string(),
    ];
    if let Some(arguments) = &a.version.arguments {
        for argument in &arguments.jvm {
            jvm.extend(expand(argument, &vars));
        }
    }
    if !jvm.iter().any(|x| x.starts_with("-Djava.library.path=")) {
        jvm.push(format!("-Djava.library.path={}", path_str(a.natives_dir)));
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
            game.extend(legacy.split_whitespace().map(|part| substitute(part, &vars)));
        }
    }

    let main_class = a
        .version
        .main_class
        .clone()
        .ctx("у версії не вказано mainClass")?;

    let log = std::fs::File::create(&paths.game_log)?;
    let log_err = log.try_clone()?;

    let mut command = Command::new(a.java);
    command
        .args(&jvm)
        .arg(&main_class)
        .args(&game)
        .current_dir(&paths.mc)
        .stdin(Stdio::null())
        .stdout(Stdio::from(log))
        .stderr(Stdio::from(log_err));
    hide_window(&mut command);

    let mut child = command.spawn().ctx("не вдалося запустити Java")?;
    let _ = app.emit(
        "game-state",
        GameState {
            state: "running".to_string(),
            code: None,
        },
    );

    let handle = app.clone();
    tauri::async_runtime::spawn(async move {
        let code = match child.wait().await {
            Ok(exit) => exit.code(),
            Err(_) => None,
        };
        let _ = handle.emit(
            "game-state",
            GameState {
                state: "exited".to_string(),
                code,
            },
        );
    });
    Ok(())
}

pub async fn run(
    app: &AppHandle,
    client: &Client,
    paths: &Paths,
    manifest: &Manifest,
    nickname: &str,
    ram_mb: u32,
) -> Result<()> {
    let mc = manifest.minecraft_version.trim().to_string();
    paths.ensure_dirs().await?;

    status(app, &format!("Підготовка Minecraft {mc}…"));
    let vanilla = minecraft::load_vanilla(app, client, paths, &mc).await?;

    let java_major = manifest
        .java_major
        .or_else(|| vanilla.java_version.as_ref().map(|j| j.major_version))
        .unwrap_or(8);
    let java_path = java::ensure_java(app, client, paths, java_major).await?;

    let client_jar = minecraft::ensure_client_jar(app, client, paths, &vanilla, &mc).await?;
    let loader_id = loaders::prepare_loader(app, client, paths, manifest, &java_path).await?;

    let version = if loader_id == mc {
        vanilla
    } else {
        let child = minecraft::read_version(paths, &loader_id).await?;
        merge(vanilla, child)
    };

    let natives_dir = paths.version_dir(&version.id).join("natives");
    let classpath =
        minecraft::install_libraries(app, client, paths, &version, &natives_dir).await?;
    let assets = minecraft::install_assets(app, client, paths, &version).await?;

    status(app, "Запуск гри…");
    spawn_game(
        app,
        paths,
        &SpawnArgs {
            version: &version,
            java: &java_path,
            classpath: &classpath,
            client_jar: &client_jar,
            natives_dir: &natives_dir,
            assets: &assets,
            nickname,
            ram_mb,
            mc: &mc,
        },
    )?;
    status(app, "Гра запущена");
    Ok(())
}
