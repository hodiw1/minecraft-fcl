//! Danger Zone у налаштуваннях: скидання даних лаунчера та повне видалення.
//!
//! Видаляються ЛИШЕ відомі дочірні папки лаунчера (`instances`, `minecraft`, `runtime`,
//! `cache`, `projects`) усередині відомих коренів (AppData лаунчера й обраної гравцем папки
//! даних). Кожна ціль перевіряється: ім'я без роздільників, канонічний шлях лежить усередині
//! кореня, а символьні посилання / junction видаляються як посилання — без переходу за ними.
//! Тож навіть якщо гравець колись обрав `D:\` папкою даних, решту диска ніхто не зачепить.

use std::path::{Path, PathBuf};

use crate::error::{fail, AppError, Result};
use crate::paths::{Paths, DATA_CHILDREN};
use crate::t;

/// Кеш індексів проєктів у корені лаунчера (відновлюється з мережі).
const ROOT_CACHES: [&str; 1] = ["projects"];

/// Імена, які взагалі можна видаляти функцією [`remove_child`].
fn is_known_child(name: &str) -> bool {
    DATA_CHILDREN.contains(&name) || ROOT_CACHES.contains(&name)
}

/// Що видаляє «Видалити все, окрім акаунту та проєктів»: (корінь, ім'я дочірньої папки).
pub fn reset_targets(paths: &Paths) -> Vec<(PathBuf, &'static str)> {
    let mut out: Vec<(PathBuf, &'static str)> = Vec::new();
    let mut push = |base: &Path, name: &'static str| {
        if !out.iter().any(|(b, n)| b == base && *n == name) {
            out.push((base.to_path_buf(), name));
        }
    };
    for child in DATA_CHILDREN {
        push(&paths.data, child);
        // Залишки в AppData після перенесення папки даних.
        push(&paths.root, child);
    }
    for child in ROOT_CACHES {
        push(&paths.root, child);
    }
    out
}

/// Видаляє `base/name`, лише якщо це справді безпосередня дочірня папка `base` з відомим іменем.
/// `Ok(false)` — нічого видаляти (папки немає).
pub fn remove_child(base: &Path, name: &str) -> Result<bool> {
    if name.is_empty() || name.contains(['/', '\\', ':']) || name == "." || name == ".." || !is_known_child(name) {
        return fail(format!("refusing to delete {name:?}"));
    }
    let target = base.join(name);
    let meta = match std::fs::symlink_metadata(&target) {
        Ok(meta) => meta,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error.into()),
    };
    if meta.file_type().is_symlink() {
        // Посилання (symlink / junction) прибираємо саме як посилання — вміст цілі не чіпаємо.
        std::fs::remove_dir(&target).or_else(|_| std::fs::remove_file(&target))?;
        return Ok(true);
    }
    let canon_base = base.canonicalize()?;
    let canon_target = target.canonicalize()?;
    if canon_target == canon_base || !canon_target.starts_with(&canon_base) {
        return fail(format!("refusing to delete {}", target.display()));
    }
    if meta.is_dir() {
        std::fs::remove_dir_all(&target)
    } else {
        std::fs::remove_file(&target)
    }
    .map_err(|error| AppError(t!("Не вдалося видалити {}: {}", "Couldn't delete {}: {}", target.display(), error)))?;
    Ok(true)
}

/// Видаляє всі цілі (по можливості — усі, навіть якщо одна не вдалася). Повертає кількість видалених.
pub async fn wipe(targets: Vec<(PathBuf, &'static str)>) -> Result<usize> {
    tokio::task::spawn_blocking(move || {
        let mut removed = 0usize;
        let mut first_error: Option<AppError> = None;
        for (base, name) in targets {
            match remove_child(&base, name) {
                Ok(true) => removed += 1,
                Ok(false) => {}
                Err(error) => {
                    first_error.get_or_insert(error);
                }
            }
        }
        match first_error {
            Some(error) => Err(error),
            None => Ok(removed),
        }
    })
    .await?
}

/// Корінь даних лаунчера (`%APPDATA%\<identifier>` / `%LOCALAPPDATA%\<identifier>`):
/// остання частина шляху дорівнює ідентифікатору застосунку і це не корінь диска.
pub fn is_launcher_root(path: &Path, identifier: &str) -> bool {
    !identifier.trim().is_empty()
        && path.is_absolute()
        && path.components().count() >= 3
        && path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.eq_ignore_ascii_case(identifier))
}

/// Повністю видаляє корінь лаунчера (AppData), якщо це справді він.
pub async fn remove_root(path: PathBuf, identifier: &str) -> Result<()> {
    if !is_launcher_root(&path, identifier) {
        return fail(format!("refusing to delete {}", path.display()));
    }
    tokio::task::spawn_blocking(move || match std::fs::symlink_metadata(&path) {
        Ok(meta) if meta.file_type().is_symlink() => std::fs::remove_dir(&path).or_else(|_| std::fs::remove_file(&path)),
        Ok(_) => std::fs::remove_dir_all(&path),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    })
    .await??;
    Ok(())
}

/// Шлях у подвійних лапках для `cmd.exe`. `None` — шлях містить символи, які cmd
/// інтерпретує навіть у лапках (`%`, `"`, керівні), — такий крок пропускаємо.
fn cmd_quote(path: &Path) -> Option<String> {
    let text = path.to_str()?;
    if text.is_empty() || text.chars().any(|c| c == '"' || c == '%' || c.is_control()) {
        return None;
    }
    Some(format!("\"{text}\""))
}

/// Скрипт для фонового `cmd.exe`, який доробляє видалення після виходу лаунчера:
/// чекає ~2 с, видаляє (або деінсталює) `.exe` і папки даних (WebView2/LocalAppData, AppData).
/// `dirs` мають бути заздалегідь перевірені [`is_launcher_root`].
pub fn cleanup_script(exe: Option<&Path>, uninstaller: Option<&Path>, dirs: &[&Path]) -> String {
    // `timeout` без консолі (CREATE_NO_WINDOW) одразу завершується з помилкою, тож
    // запасна пауза — `ping` до localhost.
    let mut steps = vec!["(timeout /t 2 /nobreak >nul 2>&1 || ping -n 3 127.0.0.1 >nul)".to_string()];
    match (uninstaller.and_then(cmd_quote), exe.and_then(cmd_quote)) {
        // Встановлена версія: штатний NSIS-деінсталятор у тихому режимі прибирає ярлики й запис у «Програмах».
        (Some(uninstaller), _) => steps.push(format!("{uninstaller} /S")),
        (None, Some(exe)) => steps.push(format!("del /f /q {exe} >nul 2>&1")),
        (None, None) => {}
    }
    if let Some(old) = exe
        .and_then(|exe| {
            exe.file_stem()
                .and_then(|stem| stem.to_str())
                .map(|stem| exe.with_file_name(format!("{stem}.old.exe")))
        })
        .as_deref()
        .and_then(cmd_quote)
    {
        steps.push(format!("del /f /q {old} >nul 2>&1"));
    }
    let quoted: Vec<String> = dirs.iter().filter_map(|dir| cmd_quote(dir)).collect();
    if !quoted.is_empty() {
        // Процеси WebView2 можуть ще мить тримати файли — друга спроба через паузу.
        for dir in &quoted {
            steps.push(format!("rmdir /s /q {dir} >nul 2>&1"));
        }
        steps.push("ping -n 3 127.0.0.1 >nul".to_string());
        for dir in &quoted {
            steps.push(format!("rmdir /s /q {dir} >nul 2>&1"));
        }
    }
    steps.join(" & ")
}

/// Запускає `cleanup_script` у фоновому `cmd.exe` без вікна консолі й не чекає на нього.
#[cfg(windows)]
pub fn spawn_cleanup(script: &str) -> Result<()> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
    std::process::Command::new("cmd.exe")
        .raw_arg(format!("/d /c {script}"))
        // Робоча папка — %TEMP%: cmd не повинен тримати папку, яку видаляє.
        .current_dir(std::env::temp_dir())
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .creation_flags(CREATE_NO_WINDOW | CREATE_NEW_PROCESS_GROUP)
        .spawn()?;
    Ok(())
}

#[cfg(not(windows))]
pub fn spawn_cleanup(_script: &str) -> Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("terrapolis-danger-test-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn only_known_children_are_removed() {
        let base = temp_dir("known");
        std::fs::create_dir_all(base.join("instances/a/mods")).unwrap();
        std::fs::write(base.join("instances/a/mods/x.jar"), b"x").unwrap();
        std::fs::create_dir_all(base.join("skins")).unwrap();
        std::fs::write(base.join("config.json"), b"{}").unwrap();

        assert!(remove_child(&base, "instances").unwrap());
        assert!(!base.join("instances").exists());
        assert!(!remove_child(&base, "runtime").unwrap(), "missing folder is not an error");
        for name in ["skins", "config.json", "..", ".", "", "instances/../skins", "C:", "projects\\..\\skins"] {
            assert!(remove_child(&base, name).is_err(), "{name}");
        }
        assert!(base.join("skins").exists());
        assert!(base.join("config.json").exists());
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn launcher_root_is_recognized() {
        assert!(is_launcher_root(Path::new("C:\\Users\\a\\AppData\\Roaming\\com.custom.mclauncher"), "com.custom.mclauncher"));
        assert!(!is_launcher_root(Path::new("C:\\"), "com.custom.mclauncher"));
        assert!(!is_launcher_root(Path::new("C:\\Users\\a\\AppData\\Roaming"), "com.custom.mclauncher"));
        assert!(!is_launcher_root(Path::new("relative\\com.custom.mclauncher"), "com.custom.mclauncher"));
        assert!(!is_launcher_root(Path::new("C:\\x\\com.custom.mclauncher"), ""));
    }

    #[test]
    fn cleanup_script_quotes_paths_and_skips_unsafe_ones() {
        let exe = Path::new("C:\\Users\\Tom&Jerry\\Apps\\Terrapolis.exe");
        let local = Path::new("C:\\Users\\Tom&Jerry\\AppData\\Local\\com.custom.mclauncher");
        let script = cleanup_script(Some(exe), None, &[local]);
        assert!(script.contains("del /f /q \"C:\\Users\\Tom&Jerry\\Apps\\Terrapolis.exe\""));
        assert!(script.contains("rmdir /s /q \"C:\\Users\\Tom&Jerry\\AppData\\Local\\com.custom.mclauncher\""));
        assert!(script.contains("Terrapolis.old.exe"));

        let installed = cleanup_script(Some(exe), Some(Path::new("C:\\Apps\\Terrapolis\\uninstall.exe")), &[]);
        assert!(installed.contains("\"C:\\Apps\\Terrapolis\\uninstall.exe\" /S"));
        assert!(!installed.contains("del /f /q \"C:\\Users\\Tom&Jerry\\Apps\\Terrapolis.exe\""));

        let weird = cleanup_script(Some(Path::new("C:\\a%PATH%\\x.exe")), None, &[]);
        assert!(!weird.contains("%PATH%"));
        let dev = cleanup_script(None, None, &[local]);
        assert!(!dev.contains("del /f"));
    }
}
