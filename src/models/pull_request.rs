use serde::{Deserialize, Serialize};

use super::label::Label;
use super::repository::Repository;
use super::user::User;

pub type PullRequestHead = super::response::ApiResponse<PullRequestHeadFields>;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PullRequestHeadFields {
    #[serde(default)]
    pub label: Option<String>,
    #[serde(rename = "ref")]
    pub ref_name: String,
    #[serde(default)]
    pub sha: Option<String>,
    #[serde(default)]
    pub repo: Option<Repository>,
}

pub type PullRequest = super::response::ApiResponse<PullRequestFields>;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PullRequestFields {
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
    pub diff_url: Option<String>,
    #[serde(default)]
    pub patch_url: Option<String>,
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
    #[serde(default)]
    pub draft: Option<bool>,
    #[serde(default)]
    pub labels: Option<Vec<Label>>,
    #[serde(default)]
    pub assignees: Option<Vec<User>>,
    #[serde(default)]
    pub merged_by: Option<User>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreatePullRequest {
    pub title: String,
    pub head: String,
    pub base: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
}

/// Only fields implemented by the GitBucket pull-request PATCH endpoint.
#[derive(Debug, Serialize)]
pub struct UpdatePullRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state: Option<String>,
}

impl UpdatePullRequest {
    pub fn is_empty(&self) -> bool {
        self.title.is_none() && self.body.is_none() && self.state.is_none()
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MergePullRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub commit_message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub merge_method: Option<String>,
}

pub type MergeResult = super::response::ApiResponse<MergeResultFields>;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MergeResultFields {
    #[serde(default)]
    pub sha: Option<String>,
    #[serde(default)]
    pub merged: Option<bool>,
    #[serde(default)]
    pub message: Option<String>,
}
