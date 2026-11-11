use std::collections::HashMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct AuthConfig {
    #[serde(default)]
    pub hosts: HashMap<String, HostConfig>,
    #[serde(default)]
    pub default_host: Option<String>,
    #[serde(default)]
    pub profiles: HashMap<String, ProfileConfig>,
    #[serde(default)]
    pub default_profile: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct HostConfig {
    pub token: String,
    pub user: String,
    #[serde(default = "default_protocol")]
    pub protocol: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct ProfileConfig {
    #[serde(default)]
    pub default_host: Option<String>,
    #[serde(default)]
    pub default_repo: Option<String>,
    #[serde(default)]
    pub hosts: HashMap<String, HostConfig>,
}

#[derive(Clone, Debug)]
pub enum Scope {
    Global,
    Profile(String),
}

impl Scope {
    pub fn profile(&self) -> Option<&str> {
        match self {
            Self::Global => None,
            Self::Profile(name) => Some(name),
        }
    }
}

#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CredentialSource {
    Environment,
    Profile,
    Global,
}

pub struct ResolvedCredential {
    pub instance: String,
    pub host: HostConfig,
    pub source: CredentialSource,
}

pub(super) fn default_protocol() -> String {
    "https".to_string()
}
