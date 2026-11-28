use std::ffi::OsString;
use std::path::{Path, PathBuf};

use crate::error::{fail, Context, Result};
use crate::i18n::tr;

/// Відкриває папку у Провіднику Windows (або файловому менеджері на інших ОС).
///
/// Раніше тут запускався окремий процес `explorer.exe <шлях>`: новий екземпляр Провідника
/// шукає вже запущену оболонку через широкомовний DDE-запит до всіх вікон верхнього рівня
/// й передає їй шлях — саме це давало ~2–3 с затримки до появи вікна. Тепер шлях передається
/// оболонці напряму (`ShellExecuteExW`, дієслово `open`, COM без DDE) у власному STA-потоці,
/// тож команда повертається миттєво й не блокує ні async-рантайм, ні головний потік вікна.
pub fn open_folder(path: &Path) -> Result<()> {
    std::fs::create_dir_all(path)?;
    shell_open(path.as_os_str().to_owned())
}

/// Відкриває вже перевірене посилання https у браузері за замовчуванням.
pub fn open_url(url: &str) -> Result<()> {
    if !url.starts_with("https://") {
        return fail(tr("некоректне посилання", "invalid link"));
    }
    shell_open(OsString::from(url))
}

#[cfg(windows)]
fn shell_open(target: OsString) -> Result<()> {
    std::thread::Builder::new()
        .name("shell-open".into())
        .spawn(move || {
            if native_shell::open(&target).is_err() {
                // Запасний шлях (вимкнена оболонка, політики): старий спосіб без очікування.
                let _ = std::process::Command::new("explorer.exe").arg(&target).spawn();
            }
        })
        .ctx(tr("не вдалося відкрити папку", "failed to open the folder"))?;
    Ok(())
}

#[cfg(not(windows))]
fn shell_open(target: OsString) -> Result<()> {
    #[cfg(target_os = "macos")]
    let program = "open";
    #[cfg(not(target_os = "macos"))]
    let program = "xdg-open";
    std::process::Command::new(program)
        .arg(target)
        .spawn()
        .ctx(tr("не вдалося відкрити папку", "failed to open the folder"))?;
    Ok(())
}

#[cfg(windows)]
mod native_shell {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;

    use windows::core::{w, PCWSTR};
    use windows::Win32::System::Com::{
        CoInitializeEx, CoUninitialize, COINIT_APARTMENTTHREADED, COINIT_DISABLE_OLE1DDE,
    };
    use windows::Win32::UI::Shell::{
        ShellExecuteExW, SEE_MASK_FLAG_NO_UI, SEE_MASK_NOASYNC, SHELLEXECUTEINFOW,
    };
    use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

    /// `ShellExecuteExW("open", target)`. Виклик іде в окремому потоці, тому `SEE_MASK_NOASYNC`
    /// (дочекатися передачі запиту оболонці до завершення потоку) — рекомендований режим.
    pub fn open(target: &OsStr) -> windows::core::Result<()> {
        let wide: Vec<u16> = target.encode_wide().chain(std::iter::once(0)).collect();
        unsafe {
            let init = CoInitializeEx(None, COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE);
            let mut info = SHELLEXECUTEINFOW {
                cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
                fMask: SEE_MASK_NOASYNC | SEE_MASK_FLAG_NO_UI,
                lpVerb: w!("open"),
                lpFile: PCWSTR(wide.as_ptr()),
                nShow: SW_SHOWNORMAL.0,
                ..Default::default()
            };
            let result = ShellExecuteExW(&mut info);
            if init.is_ok() {
                CoUninitialize();
            }
            result
        }
    }
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
    open_url(&url)
}

/// Тип файлу для діалогу вибору: підпис фільтра й розширення (без крапки; перше — типове).
pub struct FileFilter {
    pub label: String,
    pub extensions: &'static [&'static str],
}

/// Нативний діалог Windows (`IFileOpenDialog`) у власному STA-потоці.
/// Раніше діалоги відкривалися через дочірній `powershell.exe` із прихованим вікном —
/// такий патерн евристики антивірусів (зокрема Windows Defender) вважають підозрілим.
#[cfg(windows)]
mod native_dialog {
    use std::ffi::c_void;

    use windows::core::{HRESULT, HSTRING, PCWSTR};
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

    use super::FileFilter;

    pub enum Kind {
        Folder,
        File(FileFilter),
    }

    /// Показує діалог і чекає вибору. `owner` — HWND вікна лаунчера (діалог модальний до нього).
    /// `Ok(None)` — користувач натиснув «Скасувати».
    pub fn show(owner: Option<isize>, title: &str, kind: Kind) -> windows::core::Result<Option<String>> {
        unsafe {
            let init = CoInitializeEx(None, COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE);
            init.ok()?;
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
                Kind::File(filter) => {
                    options |= FOS_FILEMUSTEXIST;
                    let label = filter.label.replace(['|', '\0'], "");
                    let patterns: Vec<String> = filter.extensions.iter().map(|ext| format!("*.{ext}")).collect();
                    let name = HSTRING::from(format!("{label} ({})", patterns.join(", ")));
                    let spec = HSTRING::from(patterns.join(";"));
                    let filters = [COMDLG_FILTERSPEC {
                        pszName: PCWSTR(name.as_ptr()),
                        pszSpec: PCWSTR(spec.as_ptr()),
                    }];
                    dialog.SetFileTypes(&filters)?;
                    dialog.SetFileTypeIndex(1)?;
                    if let Some(first) = filter.extensions.first() {
                        dialog.SetDefaultExtension(&HSTRING::from(*first))?;
                    }
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

/// Запускає нативний діалог в окремому STA-потоці (COM потребує STA), не блокуючи
/// ні async-рантайм Tauri, ні головний потік вікна: інтерфейс лишається живим, поки діалог відкрито.
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

/// Стандартний діалог вибору файлу (.zip збірки, .png скіна) — як і вибір папки, без плагінів Tauri.
pub async fn pick_file(owner: Option<isize>, title: &str, filter: FileFilter) -> Result<Option<String>> {
    #[cfg(windows)]
    {
        run_dialog(
            owner,
            title,
            native_dialog::Kind::File(filter),
            tr("не вдалося відкрити вибір файлу", "failed to open the file picker"),
        )
        .await
    }
    #[cfg(not(windows))]
    {
        let _ = (owner, title, filter);
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
