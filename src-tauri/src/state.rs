use reqwest::Client;

use crate::util::http_client;

pub struct AppState {
    pub client: Client,
    pub busy: tokio::sync::Mutex<()>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            client: http_client().expect("не вдалося створити HTTP-клієнт"),
            busy: tokio::sync::Mutex::new(()),
        }
    }
}
