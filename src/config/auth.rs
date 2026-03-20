use std::collections::HashMap;
use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::config::ensure_config_dir;
use crate::error::{GbError, Result};

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct AuthConfig {
    #[serde(default)]
    pub hosts: HashMap<String, HostConfig>,
    #[serde(default)]
    pub default_host: Option<String>,
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

fn protocol_from_hostname(hostname: &str) -> Option<String> {
    hostname
        .split_once("://")
        .map(|(protocol, _)| protocol.to_string())
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
        write_config_file(&path, &content)?;
        Ok(())
    }

    /// Get host config, checking environment variable first
    pub fn get_host(&self, hostname: &str) -> Result<HostConfig> {
        // Check environment variable first
        if let Ok(token) = std::env::var("GB_TOKEN") {
            return Ok(HostConfig {
                token,
                user: String::new(),
                protocol: protocol_from_hostname(hostname)
                    .or_else(|| std::env::var("GB_PROTOCOL").ok())
                    .unwrap_or_else(default_protocol),
            });
        }

        self.hosts
            .get(hostname)
            .cloned()
            .ok_or(GbError::NotAuthenticated)
    }

    /// Add or update host config
    pub fn set_host(&mut self, hostname: String, config: HostConfig) {
        self.default_host = Some(hostname.clone());
        self.hosts.insert(hostname, config);
    }

    /// Remove host config
    pub fn remove_host(&mut self, hostname: &str) -> bool {
        let removed = self.hosts.remove(hostname).is_some();
        if removed && self.default_host.as_deref() == Some(hostname) {
            self.default_host = sorted_hostnames(&self.hosts).into_iter().next();
        }
        removed
    }

    /// Get the default hostname from env or first configured host
    pub fn default_hostname(&self) -> Option<String> {
        if let Ok(host) = std::env::var("GB_HOST") {
            return Some(host);
        }
        if let Some(host) = self
            .default_host
            .as_ref()
            .filter(|host| self.hosts.contains_key(host.as_str()))
        {
            return Some(host.clone());
        }
        sorted_hostnames(&self.hosts).into_iter().next()
    }
}

fn sorted_hostnames(hosts: &HashMap<String, HostConfig>) -> Vec<String> {
    let mut hostnames: Vec<String> = hosts.keys().cloned().collect();
    hostnames.sort();
    hostnames
}

fn write_config_file(path: &Path, content: &str) -> Result<()> {
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

#[cfg(test)]
mod tests {
    use super::{protocol_from_hostname, write_config_file, AuthConfig, HostConfig};
    use std::collections::HashMap;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn host(user: &str) -> HostConfig {
        HostConfig {
            token: "token".into(),
            user: user.into(),
            protocol: "https".into(),
        }
    }

    fn temp_path(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("gb-tests-{name}-{}-{nanos}", std::process::id()))
    }

    #[test]
    fn set_host_updates_default_host() {
        let mut config = AuthConfig::default();

        config.set_host("gitbucket.example.com".into(), host("alice"));

        assert_eq!(
            config.default_hostname().as_deref(),
            Some("gitbucket.example.com")
        );
    }

    #[test]
    fn default_hostname_prefers_explicit_default() {
        let mut config = AuthConfig {
            hosts: HashMap::new(),
            default_host: Some("b.example.com".into()),
        };
        config.hosts.insert("a.example.com".into(), host("alice"));
        config.hosts.insert("b.example.com".into(), host("bob"));

        assert_eq!(config.default_hostname().as_deref(), Some("b.example.com"));
    }

    #[test]
    fn default_hostname_falls_back_to_sorted_hostnames() {
        let mut config = AuthConfig::default();
        config.hosts.insert("z.example.com".into(), host("zoe"));
        config.hosts.insert("a.example.com".into(), host("alice"));

        assert_eq!(config.default_hostname().as_deref(), Some("a.example.com"));
    }

    #[test]
    fn protocol_can_be_derived_from_hostname() {
        assert_eq!(
            protocol_from_hostname("http://localhost:8080/gitbucket").as_deref(),
            Some("http")
        );
        assert_eq!(
            protocol_from_hostname("https://gitbucket.example.com").as_deref(),
            Some("https")
        );
        assert_eq!(protocol_from_hostname("gitbucket.example.com"), None);
    }

    #[test]
    fn remove_host_promotes_next_sorted_host() {
        let mut config = AuthConfig {
            hosts: HashMap::new(),
            default_host: Some("b.example.com".into()),
        };
        config.hosts.insert("a.example.com".into(), host("alice"));
        config.hosts.insert("b.example.com".into(), host("bob"));

        assert!(config.remove_host("b.example.com"));
        assert_eq!(config.default_hostname().as_deref(), Some("a.example.com"));
    }

    #[cfg(unix)]
    #[test]
    fn save_uses_private_file_permissions() {
        use std::os::unix::fs::PermissionsExt;

        let dir = temp_path("config-dir");
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.toml");

        write_config_file(
            &path,
            "token = 'secret'
",
        )
        .unwrap();

        let mode = fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);

        let _ = fs::remove_file(&path);
        let _ = fs::remove_dir(&dir);
    }
}
