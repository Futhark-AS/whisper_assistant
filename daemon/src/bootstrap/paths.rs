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

#[cfg(test)]
mod tests {
    use super::AppPaths;

    #[test]
    fn resolve_sets_expected_contract_filenames() {
        let paths = AppPaths::resolve().expect("resolve");
        assert_eq!(
            paths
                .config_file
                .file_name()
                .and_then(|v| v.to_str())
                .expect("filename"),
            "config.toml"
        );
        assert_eq!(
            paths
                .history_db
                .file_name()
                .and_then(|v| v.to_str())
                .expect("filename"),
            "history.sqlite3"
        );

        if cfg!(target_os = "macos") {
            assert_eq!(
                paths
                    .autostart_file
                    .file_name()
                    .and_then(|v| v.to_str())
                    .expect("filename"),
                "io.quedo.daemon.plist"
            );
        } else {
            assert_eq!(
                paths
                    .autostart_file
                    .file_name()
                    .and_then(|v| v.to_str())
                    .expect("filename"),
                "quedo-daemon.desktop"
            );
        }
    }

    #[test]
    fn ensure_dirs_creates_expected_hierarchy() {
        let temp_dir = tempfile::TempDir::new().expect("tempdir");
        let autostart_file = if cfg!(target_os = "macos") {
            temp_dir
                .path()
                .join("Library")
                .join("LaunchAgents")
                .join("io.quedo.daemon.plist")
        } else {
            temp_dir
                .path()
                .join(".config")
                .join("autostart")
                .join("quedo-daemon.desktop")
        };

        let paths = AppPaths {
            config_dir: temp_dir.path().join("config"),
            data_dir: temp_dir.path().join("data"),
            cache_dir: temp_dir.path().join("cache"),
            logs_dir: temp_dir.path().join("cache").join("logs"),
            state_dir: temp_dir.path().join("cache").join("fw-state"),
            config_file: temp_dir.path().join("config").join("config.toml"),
            history_db: temp_dir.path().join("data").join("history.sqlite3"),
            autostart_file,
        };

        paths.ensure_dirs().expect("ensure dirs");

        assert!(paths.config_dir.is_dir());
        assert!(paths.data_dir.is_dir());
        assert!(paths.cache_dir.is_dir());
        assert!(paths.logs_dir.is_dir());
        assert!(paths.state_dir.is_dir());
        assert!(paths
            .autostart_file
            .parent()
            .expect("autostart parent")
            .is_dir());
    }
}
