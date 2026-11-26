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

fn is_url_part(value: &str) -> bool {
    !value.is_empty()
        && !value.contains("..")
        && value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '+'))
}

/// Відкриває у браузері репозиторій, реліз або коміт. URL складається тут із
/// перевірених частин, тож інтерфейс не може відкрити довільну адресу.
pub fn open_github(repo: &str, kind: &str, tag: &str) -> Result<()> {
    let Some((owner, name)) = repo
        .split_once('/')
        .filter(|(owner, name)| is_url_part(owner) && is_url_part(name))
    else {
        return fail(tr("некоректний репозиторій", "invalid repository"));
    };
    let mut url = format!("https://github.com/{owner}/{name}");
    match kind {
        "release" if is_url_part(tag) => url.push_str(&format!("/releases/tag/{tag}")),
        "release" => url.push_str("/releases"),
        "commit" if is_url_part(tag) => url.push_str(&format!("/commit/{tag}")),
        _ => {}
    }
    #[cfg(windows)]
    let program = "explorer.exe";
    #[cfg(target_os = "macos")]
    let program = "open";
    #[cfg(all(unix, not(target_os = "macos")))]
    let program = "xdg-open";
    std::process::Command::new(program)
        .arg(url)
        .spawn()
        .ctx(tr("не вдалося відкрити браузер", "failed to open the browser"))?;
    Ok(())
}

/// Нативний діалог Windows (`IFileOpenDialog`) у власному STA-потоці.
/// Раніше діалоги відкривалися через дочірній `powershell.exe` із прихованим вікном —
/// такий патерн евристики антивірусів (зокрема Windows Defender) вважають підозрілим.
#[cfg(windows)]
mod native_dialog {
    use std::ffi::c_void;

    use windows::core::{w, HRESULT, HSTRING, PCWSTR};
    use windows::Win32::Foundation::{ERROR_CANCELLED, HWND};
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CoTaskMemFree, CoUninitialize, CLSCTX_INPROC_SERVER,
        COINIT_APARTMENTTHREADED, COINIT_DISABLE_OLE1DDE,
    };
    use windows::Win32::UI::Shell::Common::COMDLG_FILTERSPEC;
    use windows::Win32::UI::Shell::{
        FileOpenDialog, IFileOpenDialog, FOS_FILEMUSTEXIST, FOS_FORCEFILESYSTEM, FOS_NOCHANGEDIR,
        FOS_PATHMUSTEXIST, FOS_PICKFOLDERS, SIGDN_FILESYSPATH,
    };

    pub enum Kind {
        Folder,
        Zip { label: String },
    }

    /// Показує діалог і чекає вибору. `owner` — HWND вікна лаунчера (діалог модальний до нього).
    /// `Ok(None)` — користувач натиснув «Скасувати».
    pub fn show(owner: Option<isize>, title: &str, kind: Kind) -> windows::core::Result<Option<String>> {
        unsafe {
            CoInitializeEx(None, COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE).ok()?;
            let result = run(owner, title, kind);
            CoUninitialize();
            result
        }
    }

    unsafe fn run(owner: Option<isize>, title: &str, kind: Kind) -> windows::core::Result<Option<String>> {
        unsafe {
            let dialog: IFileOpenDialog = CoCreateInstance(&FileOpenDialog, None, CLSCTX_INPROC_SERVER)?;
            let mut options = dialog.GetOptions()? | FOS_FORCEFILESYSTEM | FOS_PATHMUSTEXIST | FOS_NOCHANGEDIR;
            dialog.SetTitle(&HSTRING::from(title))?;
            match kind {
                Kind::Folder => options |= FOS_PICKFOLDERS,
                Kind::Zip { label } => {
                    options |= FOS_FILEMUSTEXIST;
                    let name = HSTRING::from(format!("{label} (*.zip)"));
                    let spec = HSTRING::from("*.zip");
                    let filters = [COMDLG_FILTERSPEC {
                        pszName: PCWSTR(name.as_ptr()),
                        pszSpec: PCWSTR(spec.as_ptr()),
                    }];
                    dialog.SetFileTypes(&filters)?;
                    dialog.SetFileTypeIndex(1)?;
                    dialog.SetDefaultExtension(w!("zip"))?;
                }
            }
            dialog.SetOptions(options)?;

            let owner = owner.map(|raw| HWND(raw as *mut c_void));
            if let Err(error) = dialog.Show(owner) {
                if error.code() == HRESULT::from_win32(ERROR_CANCELLED.0) {
                    return Ok(None);
                }
                return Err(error);
            }
            let item = dialog.GetResult()?;
            let raw = item.GetDisplayName(SIGDN_FILESYSPATH)?;
            let path = raw.to_string();
            CoTaskMemFree(Some(raw.0 as *const c_void));
            Ok(path.ok().filter(|path| !path.trim().is_empty()))
        }
    }
}

/// Запускає нативний діалог в окремому потоці (COM потребує STA), не блокуючи async-рантайм.
#[cfg(windows)]
async fn run_dialog(owner: Option<isize>, title: &str, kind: native_dialog::Kind, error_text: &str) -> Result<Option<String>> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    let title = title.to_string();
    std::thread::Builder::new()
        .name("file-dialog".into())
        .spawn(move || {
            let _ = tx.send(native_dialog::show(owner, &title, kind).map_err(|e| e.message()));
        })
        .ctx(error_text)?;
    match rx.await {
        Ok(Ok(selected)) => Ok(selected),
        Ok(Err(error)) => fail(format!("{error_text}: {error}")),
        Err(error) => fail(format!("{error_text}: {error}")),
    }
}

/// Стандартний діалог вибору папки Windows без додаткових плагінів Tauri.
/// `owner` — HWND головного вікна (як `isize`), щоб діалог був модальним і поверх лаунчера.
pub async fn pick_folder(owner: Option<isize>, title: &str) -> Result<Option<String>> {
    #[cfg(windows)]
    {
        run_dialog(
            owner,
            title,
            native_dialog::Kind::Folder,
            tr("не вдалося відкрити вибір папки", "failed to open the folder picker"),
        )
        .await
    }
    #[cfg(not(windows))]
    {
        let _ = (owner, title);
        Ok(None)
    }
}

/// Стандартний діалог вибору .zip-файлу Windows (як і вибір папки — без плагінів Tauri).
pub async fn pick_zip(owner: Option<isize>, title: &str, filter_label: &str) -> Result<Option<String>> {
    #[cfg(windows)]
    {
        run_dialog(
            owner,
            title,
            native_dialog::Kind::Zip {
                label: filter_label.replace(['|', '\0'], ""),
            },
            tr("не вдалося відкрити вибір файлу", "failed to open the file picker"),
        )
        .await
    }
    #[cfg(not(windows))]
    {
        let _ = (owner, title, filter_label);
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
