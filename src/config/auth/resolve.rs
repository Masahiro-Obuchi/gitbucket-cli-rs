use std::collections::HashMap;

use crate::error::{GbError, Result};

use super::model::{default_protocol, AuthConfig, HostConfig};

pub(crate) fn protocol_from_hostname(hostname: &str) -> Option<String> {
    hostname
        .split_once("://")
        .map(|(protocol, _)| protocol.to_string())
}

pub(crate) fn canonical_hostname(hostname: &str) -> Option<String> {
    let trimmed = hostname.trim().trim_end_matches('/');
    let without_api = trimmed.strip_suffix("/api/v3").unwrap_or(trimmed);

    if without_api.starts_with("http://") || without_api.starts_with("https://") {
        let parsed = url::Url::parse(without_api).ok()?;
        let host = parsed.host_str()?;

        let mut canonical = host.to_string();
        if let Some(port) = parsed.port() {
            canonical.push(':');
            canonical.push_str(&port.to_string());
        }

        let path = parsed.path().trim_end_matches('/');
        if !path.is_empty() && path != "/" {
            canonical.push_str(path);
        }

        Some(canonical)
    } else {
        Some(without_api.to_string())
    }
}

impl AuthConfig {
    pub fn get_host(&self, hostname: &str) -> Result<HostConfig> {
        let stored_host = self.find_host(hostname);

        if let Ok(token) = std::env::var("GB_TOKEN") {
            return Ok(HostConfig {
                token,
                user: std::env::var("GB_USER")
                    .ok()
                    .or_else(|| stored_host.map(|host| host.user.clone()))
                    .unwrap_or_default(),
                protocol: protocol_from_hostname(hostname)
                    .or_else(|| std::env::var("GB_PROTOCOL").ok())
                    .or_else(|| stored_host.map(|host| host.protocol.clone()))
                    .unwrap_or_else(default_protocol),
            });
        }

        stored_host.cloned().ok_or(GbError::NotAuthenticated)
    }

    pub fn set_host(&mut self, hostname: String, config: HostConfig) {
        self.default_host = Some(hostname.clone());
        self.hosts.insert(hostname, config);
    }

    pub fn remove_host(&mut self, hostname: &str) -> bool {
        let key_to_remove = if self.hosts.contains_key(hostname) {
            Some(hostname.to_string())
        } else {
            let canonical = canonical_hostname(hostname);
            self.hosts.keys().find_map(|key| {
                (canonical_hostname(key).as_ref() == canonical.as_ref()).then(|| key.clone())
            })
        };

        let removed = key_to_remove
            .as_ref()
            .map(|key| self.hosts.remove(key).is_some())
            .unwrap_or(false);

        if removed
            && key_to_remove
                .as_deref()
                .is_some_and(|key| self.default_host.as_deref() == Some(key))
        {
            self.default_host = sorted_hostnames(&self.hosts).into_iter().next();
        }
        removed
    }

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

    pub fn stored_hostname(&self, hostname: &str) -> Option<String> {
        if self.hosts.contains_key(hostname) {
            return Some(hostname.to_string());
        }

        let canonical = canonical_hostname(hostname)?;
        let mut matches: Vec<String> = self
            .hosts
            .keys()
            .filter(|key| canonical_hostname(key).as_deref() == Some(canonical.as_str()))
            .cloned()
            .collect();
        matches.sort();
        matches.into_iter().next()
    }

    pub(super) fn find_host(&self, hostname: &str) -> Option<&HostConfig> {
        let key = self.stored_hostname(hostname)?;
        self.hosts.get(&key)
    }
}

pub(super) fn sorted_hostnames(hosts: &HashMap<String, HostConfig>) -> Vec<String> {
    let mut hostnames: Vec<String> = hosts.keys().cloned().collect();
    hostnames.sort();
    hostnames
}

#[cfg(test)]
mod tests {
    use super::*;

    fn host_config() -> HostConfig {
        HostConfig {
            token: "tok".to_string(),
            user: "user".to_string(),
            protocol: "https".to_string(),
        }
    }

    fn config_with_hosts(keys: &[&str]) -> AuthConfig {
        let mut hosts = HashMap::new();
        for key in keys {
            hosts.insert(key.to_string(), host_config());
        }
        AuthConfig {
            hosts,
            default_host: None,
        }
    }

    #[test]
    fn stored_hostname_exact_match() {
        let config = config_with_hosts(&["example.com"]);
        assert_eq!(
            config.stored_hostname("example.com"),
            Some("example.com".to_string())
        );
    }

    #[test]
    fn stored_hostname_canonical_match() {
        let config = config_with_hosts(&["example.com"]);
        assert_eq!(
            config.stored_hostname("https://example.com"),
            Some("example.com".to_string())
        );
    }

    #[test]
    fn stored_hostname_no_match() {
        let config = config_with_hosts(&["example.com"]);
        assert_eq!(config.stored_hostname("other.com"), None);
    }

    #[test]
    fn stored_hostname_multiple_canonical_matches_returns_lexicographically_smallest() {
        // Both "example.com" and "https://example.com" canonicalize to "example.com".
        // The function should deterministically return the lexicographically smallest key.
        let config = config_with_hosts(&["https://example.com", "example.com"]);
        assert_eq!(
            config.stored_hostname("example.com"),
            Some("example.com".to_string())
        );
    }
}
