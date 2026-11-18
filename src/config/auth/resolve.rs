use std::collections::HashMap;

use super::{
    instance::{instance_candidates, matching_instance},
    model::{AuthConfig, CredentialSource, HostConfig, ProfileConfig, ResolvedCredential, Scope},
};
use crate::error::{GbError, Result};

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
    pub fn save_login(&mut self, scope: &Scope, instance: String, host: HostConfig) {
        let (hosts, default) = match scope {
            Scope::Global => (&mut self.hosts, &mut self.default_host),
            Scope::Profile(name) => {
                let profile = self.profile_mut(name);
                (&mut profile.hosts, &mut profile.default_host)
            }
        };
        *default = Some(instance.clone());
        hosts.insert(instance, host);
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
    pub fn saved_key(&self, scope: &Scope, host: &str) -> Result<Option<String>> {
        let candidates = instance_candidates(host, None)?;
        Ok(matching_instance(self.hosts_in(scope)?, &candidates)?.map(str::to_owned))
    }

    pub fn profile_default_instance(&self, profile: &str, host: &str) -> Result<String> {
        let candidates = instance_candidates(host, None)?;
        if let Some(profile) = self.profiles.get(profile) {
            if let Some(instance) = matching_instance(&profile.hosts, &candidates)? {
                return Ok(instance.to_owned());
            }
        }
        Ok(matching_instance(&self.hosts, &candidates)?
            .map(str::to_owned)
            .unwrap_or(candidates.0))
    }

    pub fn credential(&self, hostname: &str, scope: &Scope) -> Result<ResolvedCredential> {
        let protocol = std::env::var("GB_PROTOCOL").ok();
        let candidates = instance_candidates(hostname, protocol.as_deref())?;
        let hosts = self.hosts_in(scope)?;
        let source = match scope {
            Scope::Global => CredentialSource::Global,
            Scope::Profile(_) => CredentialSource::Profile,
        };
        let saved = match matching_instance(hosts, &candidates)? {
            Some(instance) => Some((instance, &hosts[instance], source)),
            None if scope.profile().is_some() => matching_instance(&self.hosts, &candidates)?
                .map(|instance| (instance, &self.hosts[instance], CredentialSource::Global)),
            None => None,
        };

        if let Ok(token) = std::env::var("GB_TOKEN") {
            if token.is_empty() {
                return Err(GbError::Config("GB_TOKEN is empty.".into()));
            }
            return Ok(ResolvedCredential {
                instance: saved
                    .map(|(instance, _, _)| instance.to_owned())
                    .unwrap_or(candidates.0),
                host: HostConfig {
                    token,
                    user: std::env::var("GB_USER").unwrap_or_default(),
                },
                source: CredentialSource::Environment,
            });
        }
        let (instance, host, source) = saved.ok_or(GbError::NotAuthenticated)?;
        if host.token.is_empty() {
            return Err(GbError::NotAuthenticated);
        }
        Ok(ResolvedCredential {
            instance: instance.to_owned(),
            host: host.clone(),
            source,
        })
    }

    pub fn credential_for_profile(
        &self,
        hostname: &str,
        profile: Option<&str>,
    ) -> Result<ResolvedCredential> {
        self.credential(hostname, &self.scope(profile, false)?)
    }

    pub fn remove_in(&mut self, scope: &Scope, hostname: &str) -> Result<bool> {
        let Some(key) = self.saved_key(scope, hostname)? else {
            return Ok(false);
        };
        self.hosts_in_mut(scope)?.remove(&key);
        if matches!(scope, Scope::Global) && self.default_host.as_deref() == Some(&key) {
            self.default_host = None;
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
        Ok(self.default_host_in(scope)?.map(str::to_owned))
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
