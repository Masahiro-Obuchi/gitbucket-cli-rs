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
    use crate::test_support::TestEnv;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

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
        let env = TestEnv::new();
        let dir = temp_dir_path("override");

        env.set("GB_CONFIG_DIR", &dir);

        assert_eq!(config_dir().unwrap(), dir);
    }

    #[test]
    fn ensure_config_dir_creates_missing_directory() {
        let env = TestEnv::new();
        let dir = temp_dir_path("create");

        if dir.exists() {
            fs::remove_dir_all(&dir).unwrap();
        }

        env.set("GB_CONFIG_DIR", &dir);

        let ensured = ensure_config_dir().unwrap();
        assert_eq!(ensured, dir);
        assert!(dir.is_dir());

        let _ = fs::remove_dir_all(&dir);
    }
}
