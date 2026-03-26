use std::collections::HashMap;

use serde::{Deserialize, Serialize};

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

pub(super) fn default_protocol() -> String {
    "https".to_string()
}
