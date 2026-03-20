pub mod auth;

use std::path::PathBuf;

use crate::error::{GbError, Result};

/// Returns the config directory path (~/.config/gb/)
pub fn config_dir() -> Result<PathBuf> {
    if let Ok(dir) = std::env::var("GB_CONFIG_DIR") {
        return Ok(PathBuf::from(dir));
    }
    let config = dirs::config_dir()
        .ok_or_else(|| GbError::Config("Could not determine config directory".to_string()))?;
    Ok(config.join("gb"))
}

/// Ensures the config directory exists
pub fn ensure_config_dir() -> Result<PathBuf> {
    let dir = config_dir()?;
    if !dir.exists() {
        std::fs::create_dir_all(&dir)?;
    }
    Ok(dir)
}
