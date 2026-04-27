use serde::{Deserialize, Serialize};

use super::user::User;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Repository {
    pub name: String,
    pub full_name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub html_url: Option<String>,
    #[serde(default)]
    pub clone_url: Option<String>,
    #[serde(rename = "private", default)]
    pub is_private: bool,
    #[serde(default)]
    pub fork: bool,
    #[serde(default)]
    pub default_branch: Option<String>,
    #[serde(default)]
    pub owner: Option<User>,
    #[serde(default)]
    pub parent: Option<RepositoryRef>,
    #[serde(default)]
    pub source: Option<RepositoryRef>,
    #[serde(default)]
    pub watchers_count: Option<u64>,
    #[serde(default)]
    pub forks_count: Option<u64>,
    #[serde(default)]
    pub open_issues_count: Option<u64>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RepositoryRef {
    pub full_name: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateRepository {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(rename = "private", skip_serializing_if = "Option::is_none")]
    pub is_private: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auto_init: Option<bool>,
}
