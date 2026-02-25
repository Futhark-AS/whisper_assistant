use std::path::PathBuf;

use crate::bootstrap::paths::AppPaths;
use crate::error::AppResult;

pub fn bootstrap_env(paths: &AppPaths) -> AppResult<PathBuf> {
    std::fs::create_dir_all(&paths.state_dir)?;

    std::env::set_var("FRANKEN_WHISPER_STATE_DIR", &paths.state_dir);
    Ok(paths.state_dir.clone())
}
