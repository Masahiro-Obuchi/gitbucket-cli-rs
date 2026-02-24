use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct User {
    pub login: String,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(rename = "type", default)]
    pub user_type: Option<String>,
    #[serde(default)]
    pub site_admin: Option<bool>,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub avatar_url: Option<String>,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub html_url: Option<String>,
}
