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

#[cfg(test)]
mod tests {
    use super::{config_dir, ensure_config_dir};
    use std::fs;
    use std::sync::{Mutex, OnceLock};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    fn temp_dir_path(name: &str) -> std::path::PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "gb-config-tests-{name}-{}-{nanos}",
            std::process::id()
        ))
    }

    #[test]
    fn config_dir_prefers_environment_override() {
        let _guard = env_lock().lock().unwrap();
        let dir = temp_dir_path("override");

        unsafe {
            std::env::set_var("GB_CONFIG_DIR", &dir);
        }

        assert_eq!(config_dir().unwrap(), dir);

        unsafe {
            std::env::remove_var("GB_CONFIG_DIR");
        }
    }

    #[test]
    fn ensure_config_dir_creates_missing_directory() {
        let _guard = env_lock().lock().unwrap();
        let dir = temp_dir_path("create");

        if dir.exists() {
            fs::remove_dir_all(&dir).unwrap();
        }

        unsafe {
            std::env::set_var("GB_CONFIG_DIR", &dir);
        }

        let ensured = ensure_config_dir().unwrap();
        assert_eq!(ensured, dir);
        assert!(dir.is_dir());

        unsafe {
            std::env::remove_var("GB_CONFIG_DIR");
        }
        let _ = fs::remove_dir_all(&dir);
    }
}
