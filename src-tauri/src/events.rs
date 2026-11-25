use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

use serde::Serialize;
use tauri::{AppHandle, Emitter};

#[derive(Clone, Serialize)]
pub struct ProgressPayload {
    pub stage: String,
    pub percent: f64,
    pub file: String,
    pub current: u64,
    pub total: u64,
}

pub fn status(app: &AppHandle, message: &str) {
    let _ = app.emit("launcher-status", message);
}

pub struct Progress {
    app: AppHandle,
    stage: String,
    total: u64,
    done: AtomicU64,
    last_bucket: AtomicU64,
    file: Mutex<String>,
}

impl Progress {
    pub fn new(app: &AppHandle, stage: &str, total: u64) -> Self {
        Self {
            app: app.clone(),
            stage: stage.to_string(),
            total,
            done: AtomicU64::new(0),
            last_bucket: AtomicU64::new(u64::MAX),
            file: Mutex::new(String::new()),
        }
    }

    pub fn set_file(&self, name: &str) {
        if let Ok(mut file) = self.file.lock() {
            *file = name.to_string();
        }
        let done = self.done.load(Ordering::SeqCst);
        self.emit(done, self.bucket(done) as f64 / 10.0);
    }

    pub fn add(&self, delta: u64) {
        let done = self.done.fetch_add(delta, Ordering::SeqCst) + delta;
        let bucket = self.bucket(done);
        if self.last_bucket.swap(bucket, Ordering::SeqCst) != bucket {
            self.emit(done, bucket as f64 / 10.0);
        }
    }

    pub fn finish(&self) {
        self.done.store(self.total, Ordering::SeqCst);
        self.emit(self.total, 100.0);
    }

    fn bucket(&self, done: u64) -> u64 {
        if self.total == 0 {
            0
        } else {
            done.min(self.total) * 1000 / self.total
        }
    }

    fn emit(&self, done: u64, percent: f64) {
        let file = self.file.lock().map(|f| f.clone()).unwrap_or_default();
        let payload = ProgressPayload {
            stage: self.stage.clone(),
            percent,
            file,
            current: done.min(self.total),
            total: self.total,
        };
        let _ = self.app.emit("sync-progress", payload);
    }
}

/// Етап встановлення/запуску для панелі прогресу:
/// manifest → files → minecraft → java → loader → libraries → assets → launch.
pub fn step(app: &AppHandle, name: &str) {
    let _ = app.emit("play-step", name);
}

/// Фаза кнопки "Грати": checking → downloading → preparing → launching.
pub fn phase(app: &AppHandle, name: &str) {
    let _ = app.emit("play-phase", name);
}
