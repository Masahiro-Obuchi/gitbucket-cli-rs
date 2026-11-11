use std::collections::HashMap;

use super::{
    instance::{instance_key, matching_keys, stored_instance, unique_key},
    model::{AuthConfig, CredentialSource, HostConfig, ProfileConfig, ResolvedCredential, Scope},
};
use crate::error::{GbError, Result};

pub(crate) fn protocol_from_hostname(hostname: &str) -> Option<String> {
    hostname
        .split_once("://")
        .map(|(scheme, _)| scheme.to_ascii_lowercase())
}

pub(crate) fn profile_name(name: &str) -> Result<String> {
    if name.is_empty() || name.trim() != name || name.chars().any(char::is_control) {
        return Err(GbError::Config(
            "Profile name cannot be empty or contain boundary whitespace/control characters."
                .into(),
        ));
    }
    Ok(name.to_string())
}

impl AuthConfig {
    pub fn save_login(&mut self, scope: &Scope, instance: &str, host: HostConfig) -> Result<()> {
        let key = match scope {
            Scope::Profile(name) if !self.profiles.contains_key(name) => None,
            _ => unique_key(self.hosts_in(scope)?, instance, None)?,
        }
        .unwrap_or_else(|| instance.to_string());
        self.set_host_for_profile(scope.profile(), key, host);
        *self.default_host_in_mut(scope)? = Some(instance.to_string());
        Ok(())
    }
    pub fn scope(&self, profile: Option<&str>, global: bool) -> Result<Scope> {
        let explicit = profile
            .map(str::to_string)
            .or_else(|| std::env::var("GB_PROFILE").ok());
        if global {
            if explicit.is_some() {
                return Err(GbError::Config(
                    "--global conflicts with --profile and GB_PROFILE.".into(),
                ));
            }
            return Ok(Scope::Global);
        }
        match explicit.or_else(|| self.default_profile.clone()) {
            Some(name) => Ok(Scope::Profile(profile_name(&name)?)),
            None => Ok(Scope::Global),
        }
    }
    pub fn hosts_in(&self, scope: &Scope) -> Result<&HashMap<String, HostConfig>> {
        match scope {
            Scope::Global => Ok(&self.hosts),
            Scope::Profile(name) => Ok(&self.profile(name)?.hosts),
        }
    }
    pub fn hosts_in_mut(&mut self, scope: &Scope) -> Result<&mut HashMap<String, HostConfig>> {
        match scope {
            Scope::Global => Ok(&mut self.hosts),
            Scope::Profile(name) => self
                .profiles
                .get_mut(name)
                .map(|p| &mut p.hosts)
                .ok_or_else(|| GbError::Config("Selected profile is not configured.".into())),
        }
    }
    pub fn default_host_in(&self, scope: &Scope) -> Result<Option<&str>> {
        match scope {
            Scope::Global => Ok(self.default_host.as_deref()),
            Scope::Profile(name) => Ok(self.profile(name)?.default_host.as_deref()),
        }
    }
    pub fn default_host_in_mut(&mut self, scope: &Scope) -> Result<&mut Option<String>> {
        match scope {
            Scope::Global => Ok(&mut self.default_host),
            Scope::Profile(name) => self
                .profiles
                .get_mut(name)
                .map(|p| &mut p.default_host)
                .ok_or_else(|| GbError::Config("Selected profile is not configured.".into())),
        }
    }
    // Management addresses only the selected table. Exact aliases remain
    // inspectable/removable so conflicting legacy entries can be repaired.
    pub fn saved_key(&self, scope: &Scope, host: &str) -> Result<Option<String>> {
        let hosts = self.hosts_in(scope)?;
        if hosts.contains_key(host) {
            return Ok(Some(host.to_string()));
        }
        unique_key(hosts, host, None)
    }
    pub fn credential(&self, hostname: &str, scope: &Scope) -> Result<ResolvedCredential> {
        let protocol =
            protocol_from_hostname(hostname).or_else(|| std::env::var("GB_PROTOCOL").ok());
        let tables = match scope {
            Scope::Global => vec![(&self.hosts, CredentialSource::Global)],
            Scope::Profile(name) => vec![
                (&self.profile(name)?.hosts, CredentialSource::Profile),
                (&self.hosts, CredentialSource::Global),
            ],
        };
        if let Ok(token) = std::env::var("GB_TOKEN") {
            if token.is_empty() {
                return Err(GbError::Config("GB_TOKEN is empty.".into()));
            }
            let mut protocol = protocol;
            if protocol.is_none() {
                for (hosts, _) in &tables {
                    let keys = matching_keys(hosts, hostname, None)?;
                    let instances: std::collections::BTreeSet<_> = keys
                        .iter()
                        .map(|key| stored_instance(key, &hosts[key]))
                        .collect::<Result<_>>()?;
                    if instances.len() > 1 {
                        return Err(GbError::Config("Specify an explicit HTTP(S) URL or GB_PROTOCOL; the saved instance scheme is ambiguous.".into()));
                    }
                    if let Some(instance) = instances.into_iter().next() {
                        protocol = protocol_from_hostname(&instance);
                        break;
                    }
                }
            }
            let instance = instance_key(hostname, protocol.as_deref().unwrap_or("https"))?;
            return Ok(ResolvedCredential {
                host: HostConfig {
                    token,
                    user: std::env::var("GB_USER").unwrap_or_default(),
                    protocol: protocol_from_hostname(&instance).expect("validated instance"),
                },
                instance,
                source: CredentialSource::Environment,
            });
        }
        for (hosts, source) in tables {
            if let Some(key) = unique_key(hosts, hostname, protocol.as_deref())? {
                let mut host = hosts[&key].clone();
                if host.token.is_empty() {
                    return Err(GbError::NotAuthenticated);
                }
                let instance = stored_instance(&key, &host)?;
                host.protocol = protocol_from_hostname(&instance).expect("validated instance");
                return Ok(ResolvedCredential {
                    instance,
                    host,
                    source,
                });
            }
        }
        Err(GbError::NotAuthenticated)
    }
    pub fn get_host_for_profile(
        &self,
        hostname: &str,
        profile: Option<&str>,
    ) -> Result<HostConfig> {
        Ok(self
            .credential(hostname, &self.scope(profile, false)?)?
            .host)
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
    pub fn remove_host(&mut self, hostname: &str) -> Result<bool> {
        self.remove_in(&Scope::Global, hostname)
    }
    pub fn remove_in(&mut self, scope: &Scope, hostname: &str) -> Result<bool> {
        let key = match self.saved_key(scope, hostname)? {
            Some(key) => key,
            None => return Ok(false),
        };
        let removed = &self.hosts_in(scope)?[&key];
        let instance = stored_instance(&key, removed)?;
        let was_default = if matches!(scope, Scope::Global) {
            self.default_host
                .as_deref()
                .map(|default| {
                    instance_key(default, &removed.protocol).map(|target| target == instance)
                })
                .transpose()?
                .unwrap_or(false)
        } else {
            false
        };
        self.hosts_in_mut(scope)?.remove(&key);
        // Profile routing survives logout, independently of its credentials.
        if matches!(scope, Scope::Global) && was_default {
            self.default_host = sorted_hostnames(&self.hosts).into_iter().next();
        }
        Ok(true)
    }
    pub fn resolve_hostname_in(
        &self,
        cli_hostname: Option<&str>,
        scope: &Scope,
    ) -> Result<Option<String>> {
        if let Some(host) = cli_hostname {
            return Ok(Some(host.to_string()));
        }
        if let Ok(host) = std::env::var("GB_HOST") {
            return Ok(Some(host));
        }
        let selected = self.default_host_in(scope)?.map(str::to_string);
        if selected.is_some() || scope.profile().is_some() {
            return Ok(selected);
        }
        Ok(sorted_hostnames(&self.hosts).into_iter().next())
    }
    pub fn resolve_hostname(
        &self,
        cli_hostname: Option<&str>,
        profile: Option<&str>,
    ) -> Result<Option<String>> {
        self.resolve_hostname_in(cli_hostname, &self.scope(profile, false)?)
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
        let Some(name) = self.active_profile_name(profile)? else {
            return Ok(None);
        };
        Ok(self.profile(&name)?.default_repo.clone())
    }
    pub fn active_profile_name(&self, profile: Option<&str>) -> Result<Option<String>> {
        let scope = self.scope(profile, false)?;
        if let Scope::Profile(name) = scope {
            self.profile(&name)?;
            Ok(Some(name))
        } else {
            Ok(None)
        }
    }
    pub fn profile(&self, profile: &str) -> Result<&ProfileConfig> {
        self.profiles.get(profile).ok_or_else(|| GbError::Config(format!(
            "Profile '{}' is not configured. Add it with `gb config set profile {}` or run `gb auth login --profile {}`.", profile, profile, profile)))
    }
    pub fn profile_mut(&mut self, profile: &str) -> &mut ProfileConfig {
        self.profiles.entry(profile.to_string()).or_default()
    }
}

pub(super) fn sorted_hostnames(hosts: &HashMap<String, HostConfig>) -> Vec<String> {
    let mut names: Vec<_> = hosts.keys().cloned().collect();
    names.sort();
    names
}
