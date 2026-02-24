use std::collections::HashMap;
use std::fs;

use serde::{Deserialize, Serialize};

use crate::config::ensure_config_dir;
use crate::error::{GbError, Result};

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct AuthConfig {
    #[serde(default)]
    pub hosts: HashMap<String, HostConfig>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct HostConfig {
    pub token: String,
    pub user: String,
    #[serde(default = "default_protocol")]
    pub protocol: String,
}

fn default_protocol() -> String {
    "https".to_string()
}

impl AuthConfig {
    /// Load auth config from file
    pub fn load() -> Result<Self> {
        let path = ensure_config_dir()?.join("config.toml");
        if !path.exists() {
            return Ok(Self::default());
        }
        let content = fs::read_to_string(&path)?;
        let config: AuthConfig = toml::from_str(&content)?;
        Ok(config)
    }

    /// Save auth config to file
    pub fn save(&self) -> Result<()> {
        let path = ensure_config_dir()?.join("config.toml");
        let content = toml::to_string_pretty(self)?;
        fs::write(&path, content)?;
        Ok(())
    }

    /// Get host config, checking environment variable first
    pub fn get_host(&self, hostname: &str) -> Result<HostConfig> {
        // Check environment variable first
        if let Ok(token) = std::env::var("GB_TOKEN") {
            return Ok(HostConfig {
                token,
                user: String::new(),
                protocol: default_protocol(),
            });
        }

        self.hosts
            .get(hostname)
            .cloned()
            .ok_or(GbError::NotAuthenticated)
    }

    /// Add or update host config
    pub fn set_host(&mut self, hostname: String, config: HostConfig) {
        self.hosts.insert(hostname, config);
    }

    /// Remove host config
    pub fn remove_host(&mut self, hostname: &str) -> bool {
        self.hosts.remove(hostname).is_some()
    }

    /// Get the default hostname from env or first configured host
    pub fn default_hostname(&self) -> Option<String> {
        if let Ok(host) = std::env::var("GB_HOST") {
            return Some(host);
        }
        self.hosts.keys().next().cloned()
    }
}
