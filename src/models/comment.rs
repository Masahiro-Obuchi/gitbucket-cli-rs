//! Ordinary Issue/PR discussion responses. Commit/inline reviews use a separate reader/model.
use serde::{Deserialize, Serialize};

use super::user::User;

pub type Comment = super::response::ApiResponse<CommentFields>;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CommentFields {
    pub id: u64,
    #[serde(default)]
    pub body: Option<String>,
    #[serde(default)]
    pub user: Option<User>,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub updated_at: Option<String>,
    #[serde(default)]
    pub html_url: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateComment {
    pub body: String,
}
