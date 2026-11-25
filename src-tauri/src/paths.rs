use std::path::PathBuf;

use tauri::{AppHandle, Manager};

use crate::config::Config;
use crate::error::{Context, Result};

/// Розкладка файлів:
///
/// ```text
/// <root>/                 AppData лаунчера: config.json, skins/, projects/ (кеш індексів)
/// <data>/                 робоча папка (за замовчуванням = root, змінюється в налаштуваннях)
///   minecraft/            СПІЛЬНЕ: versions/, libraries/, assets/ — без дублювання між збірками
///   runtime/              СПІЛЬНЕ: завантажені Java
///   cache/                інсталятори завантажувачів
///   instances/<id>/       ІЗОЛЬОВАНА збірка: mods/, config/, resourcepacks/, shaderpacks/,
///                         servers.dat, saves/, options.txt, .launcher/ (службові файли)
/// ```
#[derive(Clone)]
pub struct Paths {
    pub root: PathBuf,
    pub data: PathBuf,
    pub mc: PathBuf,
    pub runtime: PathBuf,
    pub cache: PathBuf,
    pub instances: PathBuf,
}

pub const DATA_CHILDREN: [&str; 4] = ["minecraft", "runtime", "cache", "instances"];

impl Paths {
    pub fn app_root(app: &AppHandle) -> Result<PathBuf> {
        app.path()
            .app_data_dir()
            .ctx("не вдалося визначити папку лаунчера")
    }

    pub fn build(root: PathBuf, data: PathBuf) -> Self {
        Self {
            mc: data.join("minecraft"),
            runtime: data.join("runtime"),
            cache: data.join("cache"),
            instances: data.join("instances"),
            root,
            data,
        }
    }

    pub fn resolve(app: &AppHandle, config: &Config) -> Result<Self> {
        let root = Self::app_root(app)?;
        let custom = config.data_dir.trim();
        let data = if custom.is_empty() {
            root.clone()
        } else {
            PathBuf::from(custom)
        };
        Ok(Self::build(root, data))
    }

    pub fn skins(&self) -> PathBuf {
        self.root.join("skins")
    }

    pub fn skin_file(&self, profile_id: &str) -> PathBuf {
        self.skins().join(format!("{profile_id}.png"))
    }

    pub fn projects_cache(&self) -> PathBuf {
        self.root.join("projects")
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

    pub fn instance_dir(&self, id: &str) -> PathBuf {
        self.instances.join(id)
    }

    pub fn instance_meta(&self, id: &str) -> PathBuf {
        self.instance_dir(id).join(".launcher")
    }

    pub async fn ensure_shared_dirs(&self) -> Result<()> {
        let dirs = [
            self.root.clone(),
            self.data.clone(),
            self.mc.clone(),
            self.runtime.clone(),
            self.cache.clone(),
            self.instances.clone(),
            self.versions(),
            self.libraries(),
            self.assets(),
        ];
        for dir in dirs {
            tokio::fs::create_dir_all(&dir).await?;
        }
        Ok(())
    }
}
