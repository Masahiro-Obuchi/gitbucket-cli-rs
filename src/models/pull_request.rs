use serde::{Deserialize, Serialize};

use super::repository::Repository;
use super::user::User;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PullRequestHead {
    #[serde(default)]
    pub label: Option<String>,
    #[serde(rename = "ref")]
    pub ref_name: String,
    #[serde(default)]
    pub sha: Option<String>,
    #[serde(default)]
    pub repo: Option<Repository>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PullRequest {
    pub number: u64,
    pub title: String,
    #[serde(default)]
    pub body: Option<String>,
    pub state: String,
    #[serde(default)]
    pub user: Option<User>,
    #[serde(default)]
    pub html_url: Option<String>,
    #[serde(default)]
    pub head: Option<PullRequestHead>,
    #[serde(rename = "base", default)]
    pub base: Option<PullRequestHead>,
    #[serde(default)]
    pub merged: Option<bool>,
    #[serde(default)]
    pub mergeable: Option<bool>,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub updated_at: Option<String>,
    #[serde(default)]
    pub closed_at: Option<String>,
    #[serde(default)]
    pub merged_at: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreatePullRequest {
    pub title: String,
    pub head: String,
    pub base: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MergePullRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub commit_message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sha: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub merge_method: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MergeResult {
    #[serde(default)]
    pub sha: Option<String>,
    #[serde(default)]
    pub merged: Option<bool>,
    #[serde(default)]
    pub message: Option<String>,
}
