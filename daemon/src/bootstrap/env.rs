use std::path::PathBuf;

use crate::bootstrap::paths::AppPaths;
use crate::error::AppResult;

pub fn bootstrap_env(paths: &AppPaths) -> AppResult<PathBuf> {
    std::fs::create_dir_all(&paths.state_dir)?;

    std::env::set_var("FRANKEN_WHISPER_STATE_DIR", &paths.state_dir);
    Ok(paths.state_dir.clone())
}

#[cfg(test)]
mod tests {
    use super::bootstrap_env;
    use crate::bootstrap::paths::AppPaths;

    #[test]
    fn bootstrap_env_creates_state_dir_and_sets_env() {
        let temp_dir = tempfile::TempDir::new().expect("tempdir");
        let state_dir = temp_dir.path().join("nested").join("fw-state");
        let paths = AppPaths {
            config_dir: temp_dir.path().join("config"),
            data_dir: temp_dir.path().join("data"),
            cache_dir: temp_dir.path().join("cache"),
            logs_dir: temp_dir.path().join("cache").join("logs"),
            state_dir: state_dir.clone(),
            config_file: temp_dir.path().join("config").join("config.toml"),
            history_db: temp_dir.path().join("data").join("history.sqlite3"),
            autostart_file: temp_dir.path().join("autostart").join("entry"),
        };

        let before = std::env::var_os("FRANKEN_WHISPER_STATE_DIR");
        let resolved = bootstrap_env(&paths).expect("bootstrap");
        assert_eq!(resolved, state_dir);
        assert!(state_dir.is_dir());
        assert_eq!(
            std::env::var("FRANKEN_WHISPER_STATE_DIR").ok().as_deref(),
            Some(state_dir.to_str().expect("utf8"))
        );

        match before {
            Some(value) => std::env::set_var("FRANKEN_WHISPER_STATE_DIR", value),
            None => std::env::remove_var("FRANKEN_WHISPER_STATE_DIR"),
        }
    }
}
