use serde::{Deserialize, Serialize};

use super::user::User;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Milestone {
    pub number: u64,
    pub title: String,
    pub state: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub due_on: Option<String>,
    #[serde(default)]
    pub html_url: Option<String>,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub creator: Option<User>,
    #[serde(default)]
    pub open_issues: Option<u64>,
    #[serde(default)]
    pub closed_issues: Option<u64>,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub updated_at: Option<String>,
    #[serde(default)]
    pub closed_at: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateMilestone {
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub due_on: Option<String>,
}

/// GitBucket 4.44 replaces all editable fields, rather than applying a sparse PATCH.
/// None is serialized as null to clear nullable fields.
#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateMilestone {
    pub title: String,
    pub description: Option<String>,
    pub due_on: Option<String>,
    pub state: String,
}
