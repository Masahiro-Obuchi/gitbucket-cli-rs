use serde::{Deserialize, Serialize};

use super::label::Label;
use super::milestone::Milestone;
use super::user::User;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct IssuePullRequest {
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub html_url: Option<String>,
    #[serde(default)]
    pub diff_url: Option<String>,
    #[serde(default)]
    pub patch_url: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Issue {
    pub number: u64,
    pub title: String,
    #[serde(default)]
    pub body: Option<String>,
    pub state: String,
    #[serde(default)]
    pub user: Option<User>,
    #[serde(default)]
    pub labels: Vec<Label>,
    #[serde(default)]
    pub assignees: Vec<User>,
    #[serde(default)]
    pub milestone: Option<Milestone>,
    #[serde(default)]
    pub html_url: Option<String>,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub updated_at: Option<String>,
    #[serde(default)]
    pub closed_at: Option<String>,
    #[serde(default)]
    pub comments: Option<u64>,
    #[serde(default)]
    pub pull_request: Option<IssuePullRequest>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateIssue {
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub labels: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assignees: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub milestone: Option<u64>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateIssue {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub labels: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assignees: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub milestone: Option<Option<u64>>,
}
