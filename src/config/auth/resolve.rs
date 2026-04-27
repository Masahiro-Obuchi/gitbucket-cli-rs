use std::collections::HashMap;

use crate::error::{GbError, Result};

use super::model::{default_protocol, AuthConfig, HostConfig, ProfileConfig};

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
    pub fn get_host_for_profile(
        &self,
        hostname: &str,
        profile: Option<&str>,
    ) -> Result<HostConfig> {
        let profile_name = self.active_profile_name(profile)?;
        let profile_host = profile_name
            .as_deref()
            .and_then(|name| self.profiles.get(name))
            .and_then(|profile| find_host_in(&profile.hosts, hostname));
        let stored_host = profile_host.or_else(|| self.find_host(hostname));

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

    pub fn set_host_for_profile(
        &mut self,
        profile: Option<&str>,
        hostname: String,
        config: HostConfig,
    ) {
        if let Some(profile) = profile {
            let profile = self.profiles.entry(profile.to_string()).or_default();
            profile.default_host = Some(hostname.clone());
            profile.hosts.insert(hostname, config);
        } else {
            self.set_host(hostname, config);
        }
    }

    pub fn remove_host(&mut self, hostname: &str) -> bool {
        remove_host_from(&mut self.hosts, &mut self.default_host, hostname)
    }

    pub fn remove_host_for_profile(&mut self, profile: Option<&str>, hostname: &str) -> bool {
        let Some(profile) = profile else {
            return self.remove_host(hostname);
        };
        let Some(profile) = self.profiles.get_mut(profile) else {
            return false;
        };
        remove_host_from(&mut profile.hosts, &mut profile.default_host, hostname)
    }

    pub fn resolve_hostname(
        &self,
        cli_hostname: Option<&str>,
        profile: Option<&str>,
    ) -> Result<Option<String>> {
        if let Some(host) = cli_hostname {
            return Ok(Some(host.to_string()));
        }
        if let Ok(host) = std::env::var("GB_HOST") {
            return Ok(Some(host));
        }

        if let Some(profile_name) = self.active_profile_name(profile)? {
            return Ok(self
                .profiles
                .get(&profile_name)
                .and_then(|profile| profile.default_host.clone()));
        }

        Ok(self.default_hostname_without_profile())
    }

    pub fn resolve_repo(
        &self,
        cli_repo: Option<&str>,
        profile: Option<&str>,
    ) -> Result<Option<String>> {
        if let Some(repo) = cli_repo {
            return Ok(Some(repo.to_string()));
        }
        if let Ok(repo) = std::env::var("GB_REPO") {
            return Ok(Some(repo));
        }

        let Some(profile_name) = self.active_profile_name(profile)? else {
            return Ok(None);
        };

        Ok(self
            .profiles
            .get(&profile_name)
            .and_then(|profile| profile.default_repo.clone()))
    }

    pub fn active_profile_name(&self, profile: Option<&str>) -> Result<Option<String>> {
        let selected = profile
            .map(str::to_string)
            .or_else(|| std::env::var("GB_PROFILE").ok())
            .or_else(|| self.default_profile.clone());

        let Some(selected) = selected else {
            return Ok(None);
        };
        if selected.trim().is_empty() {
            return Err(GbError::Config("Profile name cannot be empty.".into()));
        }
        if !self.profiles.contains_key(&selected) {
            return Err(GbError::Config(format!(
                "Profile '{}' is not configured. Add it with `gb config set profile {}` or run `gb auth login --profile {}`.",
                selected, selected, selected
            )));
        }
        Ok(Some(selected))
    }

    pub fn profile(&self, profile: &str) -> Result<&ProfileConfig> {
        self.profiles.get(profile).ok_or_else(|| {
            GbError::Config(format!(
                "Profile '{}' is not configured. Add it with `gb config set profile {}`.",
                profile, profile
            ))
        })
    }

    pub fn profile_mut(&mut self, profile: &str) -> &mut ProfileConfig {
        self.profiles.entry(profile.to_string()).or_default()
    }

    fn default_hostname_without_profile(&self) -> Option<String> {
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
        stored_hostname_in(&self.hosts, hostname)
    }

    pub fn stored_hostname_for_profile(&self, profile: &str, hostname: &str) -> Option<String> {
        self.profiles
            .get(profile)
            .and_then(|profile| stored_hostname_in(&profile.hosts, hostname))
    }

    pub(super) fn find_host(&self, hostname: &str) -> Option<&HostConfig> {
        find_host_in(&self.hosts, hostname)
    }
}

fn remove_host_from(
    hosts: &mut HashMap<String, HostConfig>,
    default_host: &mut Option<String>,
    hostname: &str,
) -> bool {
    let key_to_remove = if hosts.contains_key(hostname) {
        Some(hostname.to_string())
    } else {
        let canonical = canonical_hostname(hostname);
        hosts.keys().find_map(|key| {
            (canonical_hostname(key).as_ref() == canonical.as_ref()).then(|| key.clone())
        })
    };

    let removed = key_to_remove
        .as_ref()
        .map(|key| hosts.remove(key).is_some())
        .unwrap_or(false);

    if removed
        && key_to_remove
            .as_deref()
            .is_some_and(|key| default_host.as_deref() == Some(key))
    {
        *default_host = sorted_hostnames(hosts).into_iter().next();
    }
    removed
}

fn stored_hostname_in(hosts: &HashMap<String, HostConfig>, hostname: &str) -> Option<String> {
    if hosts.contains_key(hostname) {
        return Some(hostname.to_string());
    }

    let canonical = canonical_hostname(hostname)?;
    let mut matches: Vec<String> = hosts
        .keys()
        .filter(|key| canonical_hostname(key).as_deref() == Some(canonical.as_str()))
        .cloned()
        .collect();
    matches.sort();
    matches.into_iter().next()
}

fn find_host_in<'a>(
    hosts: &'a HashMap<String, HostConfig>,
    hostname: &str,
) -> Option<&'a HostConfig> {
    let key = stored_hostname_in(hosts, hostname)?;
    hosts.get(&key)
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
            ..Default::default()
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
