use std::path::{Path, PathBuf};

use crate::error::{fail, Context, Result};
use crate::i18n::tr;

/// Відкриває папку у Провіднику Windows (або файловому менеджері на інших ОС).
pub fn open_folder(path: &Path) -> Result<()> {
    std::fs::create_dir_all(path)?;
    #[cfg(windows)]
    let program = "explorer.exe";
    #[cfg(target_os = "macos")]
    let program = "open";
    #[cfg(all(unix, not(target_os = "macos")))]
    let program = "xdg-open";
    std::process::Command::new(program)
        .arg(path)
        .spawn()
        .ctx(tr("не вдалося відкрити папку", "failed to open the folder"))?;
    Ok(())
}

/// Стандартний діалог вибору папки Windows без додаткових плагінів Tauri.
pub async fn pick_folder(title: &str) -> Result<Option<String>> {
    #[cfg(windows)]
    {
        let title = title.replace('\'', "''");
        let script = format!(
            "[Console]::OutputEncoding=[Text.Encoding]::UTF8;\
             Add-Type -AssemblyName System.Windows.Forms;\
             $owner=New-Object System.Windows.Forms.Form -Property @{{TopMost=$true}};\
             $dialog=New-Object System.Windows.Forms.FolderBrowserDialog;\
             $dialog.Description='{title}';\
             $dialog.ShowNewFolderButton=$true;\
             if($dialog.ShowDialog($owner) -eq [System.Windows.Forms.DialogResult]::OK){{[Console]::Write($dialog.SelectedPath)}}"
        );
        let mut command = tokio::process::Command::new("powershell.exe");
        command.args([
            "-NoProfile",
            "-STA",
            "-NonInteractive",
            "-Command",
            script.as_str(),
        ]);
        crate::util::hide_window(&mut command);
        let output = command.output().await.ctx(tr(
            "не вдалося відкрити вибір папки",
            "failed to open the folder picker",
        ))?;
        let selected = String::from_utf8_lossy(&output.stdout).trim().to_string();
        Ok(if selected.is_empty() {
            None
        } else {
            Some(selected)
        })
    }
    #[cfg(not(windows))]
    {
        let _ = title;
        Ok(None)
    }
}

pub fn is_dir_empty(path: &Path) -> bool {
    std::fs::read_dir(path)
        .map(|mut entries| entries.next().is_none())
        .unwrap_or(true)
}

fn copy_recursive(from: &Path, to: &Path) -> Result<()> {
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let target = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_recursive(&entry.path(), &target)?;
        } else {
            std::fs::copy(entry.path(), &target)?;
        }
    }
    Ok(())
}

/// Переносить папку: спершу швидке перейменування, якщо інший диск — копіювання й видалення.
pub fn move_dir(from: &Path, to: &Path) -> Result<()> {
    if !from.exists() {
        return Ok(());
    }
    if let Some(parent) = to.parent() {
        std::fs::create_dir_all(parent)?;
    }
    if std::fs::rename(from, to).is_ok() {
        return Ok(());
    }
    copy_recursive(from, to)?;
    std::fs::remove_dir_all(from)?;
    Ok(())
}

/// Перевіряє, що в папку можна писати (немає захисту від запису / прав адміністратора).
pub fn check_writable(dir: &Path) -> Result<()> {
    std::fs::create_dir_all(dir).ctx(tr(
        "не вдалося створити папку",
        "failed to create the folder",
    ))?;
    let probe = dir.join(".launcher-write-test");
    std::fs::write(&probe, b"ok").ctx(tr(
        "у цю папку не можна записувати — обери іншу",
        "this folder is not writable — choose another one",
    ))?;
    let _ = std::fs::remove_file(probe);
    Ok(())
}

pub fn normalize_dir(raw: &str) -> Result<PathBuf> {
    let path = PathBuf::from(raw.trim());
    if raw.trim().is_empty() || !path.is_absolute() {
        return fail(tr(
            "Вкажи повний шлях до папки, наприклад D:\\Games\\Launcher",
            "Enter a full folder path, e.g. D:\\Games\\Launcher",
        ));
    }
    Ok(path)
}
