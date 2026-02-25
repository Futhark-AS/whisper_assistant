use std::path::PathBuf;

use directories::{BaseDirs, ProjectDirs};

use crate::error::{AppError, AppResult};

#[derive(Debug, Clone)]
pub struct AppPaths {
    pub config_dir: PathBuf,
    pub data_dir: PathBuf,
    pub cache_dir: PathBuf,
    pub logs_dir: PathBuf,
    pub state_dir: PathBuf,
    pub config_file: PathBuf,
    pub history_db: PathBuf,
    pub autostart_file: PathBuf,
}

impl AppPaths {
    pub fn resolve() -> AppResult<Self> {
        let project_dirs = ProjectDirs::from("io", "quedo", "quedo")
            .ok_or_else(|| AppError::Config("unable to resolve project directories".to_owned()))?;

        let config_dir = project_dirs.config_dir().to_path_buf();
        let data_dir = project_dirs.data_local_dir().to_path_buf();
        let cache_dir = project_dirs.cache_dir().to_path_buf();
        let logs_dir = cache_dir.join("logs");
        let state_dir = cache_dir.join("fw-state");

        let config_file = config_dir.join("config.toml");
        let history_db = data_dir.join("history.sqlite3");

        let base_dirs = BaseDirs::new()
            .ok_or_else(|| AppError::Config("unable to resolve base directories".to_owned()))?;
        let autostart_file = if cfg!(target_os = "macos") {
            base_dirs
                .home_dir()
                .join("Library")
                .join("LaunchAgents")
                .join("io.quedo.daemon.plist")
        } else {
            base_dirs
                .config_dir()
                .join("autostart")
                .join("quedo-daemon.desktop")
        };

        Ok(Self {
            config_dir,
            data_dir,
            cache_dir,
            logs_dir,
            state_dir,
            config_file,
            history_db,
            autostart_file,
        })
    }

    pub fn ensure_dirs(&self) -> AppResult<()> {
        std::fs::create_dir_all(&self.config_dir)?;
        std::fs::create_dir_all(&self.data_dir)?;
        std::fs::create_dir_all(&self.cache_dir)?;
        std::fs::create_dir_all(&self.logs_dir)?;
        std::fs::create_dir_all(&self.state_dir)?;
        if let Some(parent) = self.autostart_file.parent() {
            std::fs::create_dir_all(parent)?;
        }
        Ok(())
    }
}
