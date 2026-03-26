use std::fs;
use std::path::Path;

use crate::config::ensure_config_dir;
use crate::error::Result;

use super::model::AuthConfig;

impl AuthConfig {
    pub fn load() -> Result<Self> {
        let path = ensure_config_dir()?.join("config.toml");
        if !path.exists() {
            return Ok(Self::default());
        }
        let content = fs::read_to_string(&path)?;
        let config: AuthConfig = toml::from_str(&content)?;
        Ok(config)
    }

    pub fn save(&self) -> Result<()> {
        let path = ensure_config_dir()?.join("config.toml");
        let content = toml::to_string_pretty(self)?;
        write_config_file(&path, &content)?;
        Ok(())
    }
}

pub(super) fn write_config_file(path: &Path, content: &str) -> Result<()> {
    #[cfg(unix)]
    {
        use std::io::Write;
        use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};

        let mut file = fs::OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .mode(0o600)
            .open(path)?;
        file.write_all(content.as_bytes())?;
        fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
        Ok(())
    }

    #[cfg(not(unix))]
    {
        fs::write(path, content)?;
        Ok(())
    }
}
