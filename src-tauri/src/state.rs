use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use reqwest::Client;

use crate::updater::ReleaseInfo;
use crate::util::http_client;

pub struct AppState {
    pub client: Client,
    /// Одна важка операція за раз: встановлення, ремонт, запуск, видалення, перенесення папки.
    pub busy: tokio::sync::Mutex<()>,
    /// Захищає read-modify-write config.json від гонок між командами.
    pub config_lock: tokio::sync::Mutex<()>,
    /// id збірки, гра з якої зараз запущена (захист від кількох копій Minecraft).
    pub game: Arc<Mutex<Option<String>>>,
    /// Запущені процеси гри: id збірки → PID процесу Java (для «Закрити гру»).
    /// Запис зникає, коли процес завершився сам або його закрила `stop_instance`.
    pub processes: Arc<Mutex<HashMap<String, u32>>>,
    /// Зміни в mods/ власних збірок — по одній за раз.
    pub mods_lock: tokio::sync::Mutex<()>,
    /// Остання знайдена нова версія лаунчера (щоб «Завантажити» не перевіряло все заново).
    pub update: Mutex<Option<ReleaseInfo>>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            client: http_client().expect("не вдалося створити HTTP-клієнт"),
            busy: tokio::sync::Mutex::new(()),
            config_lock: tokio::sync::Mutex::new(()),
            game: Arc::new(Mutex::new(None)),
            processes: Arc::new(Mutex::new(HashMap::new())),
            mods_lock: tokio::sync::Mutex::new(()),
            update: Mutex::new(None),
        }
    }

    pub fn running_game(&self) -> Option<String> {
        self.game.lock().ok().and_then(|g| g.clone())
    }
}
