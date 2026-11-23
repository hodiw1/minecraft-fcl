use std::path::PathBuf;

use tauri::{AppHandle, Manager};

use crate::error::{Context, Result};

#[derive(Clone)]
pub struct Paths {
    pub root: PathBuf,
    pub mc: PathBuf,
    pub runtime: PathBuf,
    pub cache: PathBuf,
    pub config_file: PathBuf,
    pub manifest_cache: PathBuf,
    pub game_log: PathBuf,
}

impl Paths {
    pub fn new(app: &AppHandle) -> Result<Self> {
        let root = app
            .path()
            .app_data_dir()
            .ctx("не вдалося визначити папку лаунчера")?;
        Ok(Self {
            mc: root.join("minecraft"),
            runtime: root.join("runtime"),
            cache: root.join("cache"),
            config_file: root.join("config.json"),
            manifest_cache: root.join("manifest.json"),
            game_log: root.join("game_output.log"),
            root,
        })
    }

    pub fn versions(&self) -> PathBuf {
        self.mc.join("versions")
    }

    pub fn libraries(&self) -> PathBuf {
        self.mc.join("libraries")
    }

    pub fn assets(&self) -> PathBuf {
        self.mc.join("assets")
    }

    pub fn version_dir(&self, id: &str) -> PathBuf {
        self.versions().join(id)
    }

    pub fn version_json(&self, id: &str) -> PathBuf {
        self.version_dir(id).join(format!("{id}.json"))
    }

    pub fn version_jar(&self, id: &str) -> PathBuf {
        self.version_dir(id).join(format!("{id}.jar"))
    }

    pub async fn ensure_dirs(&self) -> Result<()> {
        let dirs = vec![
            self.root.clone(),
            self.mc.clone(),
            self.runtime.clone(),
            self.cache.clone(),
            self.versions(),
            self.libraries(),
            self.assets(),
            self.mc.join("mods"),
            self.mc.join("config"),
            self.mc.join("resourcepacks"),
            self.mc.join("shaderpacks"),
        ];
        for dir in dirs {
            tokio::fs::create_dir_all(&dir).await?;
        }
        Ok(())
    }
}
