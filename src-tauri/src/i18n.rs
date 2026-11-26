use std::sync::atomic::{AtomicBool, Ordering};

static ENGLISH: AtomicBool = AtomicBool::new(false);

pub fn set_lang(lang: &str) {
    ENGLISH.store(lang.eq_ignore_ascii_case("en"), Ordering::Relaxed);
}

pub fn is_en() -> bool {
    ENGLISH.load(Ordering::Relaxed)
}

/// Повертає текст мовою інтерфейсу лаунчера.
pub fn tr<'a>(uk: &'a str, en: &'a str) -> &'a str {
    if is_en() {
        en
    } else {
        uk
    }
}

/// `t!("Завантажено {} файлів", "Downloaded {} files", count)` — форматований переклад.
#[macro_export]
macro_rules! t {
    ($uk:literal, $en:literal $(, $arg:expr)* $(,)?) => {
        if $crate::i18n::is_en() {
            format!($en $(, $arg)*)
        } else {
            format!($uk $(, $arg)*)
        }
    };
}
