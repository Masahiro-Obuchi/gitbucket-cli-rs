use super::pagination::Collection;
use crate::api::client::ApiClient;

use crate::error::{GbError, Result};
use crate::models::comment::{Comment, CreateComment};
use crate::models::pull_request::{
    CreatePullRequest, MergePullRequest, MergeResult, PullRequest, UpdatePullRequest,
};

impl ApiClient {
    /// List pull requests for a repository
    pub async fn list_pull_requests(
        &self,
        owner: &str,
        repo: &str,
        state: &str,
    ) -> Result<Vec<PullRequest>> {
        self.collection(
            &format!("/repos/{owner}/{repo}/pulls?state={state}"),
            Collection::IssuesOrPulls,
        )
        .await
    }

    /// The pinned collection misidentifies fork head repositories. The table
    /// uses summary fields; JSON needs canonical detail for every listed PR.
    pub async fn list_repository_pull_requests(
        &self,
        owner: &str,
        repo: &str,
        state: &str,
        hydrate: bool,
    ) -> Result<Vec<PullRequest>> {
        let mut prs = self.list_pull_requests(owner, repo, state).await?;
        if hydrate {
            for pr in &mut prs {
                let actual = self.pull_request_details(owner, repo, pr.number).await?.ok_or_else(|| GbError::Other(format!("PR #{} is listed but its canonical details are unavailable; cannot verify fork identity.", pr.number)))?;
                if actual.number != pr.number {
                    return Err(GbError::Other(
                        "PR detail returned another number; no partial list was returned.".into(),
                    ));
                }
                *pr = actual;
            }
        }
        Ok(prs)
    }

    pub(crate) async fn pull_request_details(
        &self,
        owner: &str,
        repo: &str,
        number: u64,
    ) -> Result<Option<PullRequest>> {
        self.get(&format!("/repos/{owner}/{repo}/pulls/{number}"))
            .await
    }

    /// Get a single pull request
    pub async fn get_pull_request(
        &self,
        owner: &str,
        repo: &str,
        number: u64,
    ) -> Result<PullRequest> {
        let pr = self.pull_request_details(owner, repo, number).await?;
        match pr {
            Some(pr) => Ok(pr),
            None => self
                .list_pull_requests(owner, repo, "all")
                .await?
                .into_iter()
                .find(|pr| pr.number == number)
                .map(|mut pr| {
                    // Enumeration can establish that this number is a PR, but
                    // cannot establish its fork head when the detail is null.
                    // Withhold the head rather than fetch a branch from a
                    // potentially unrelated base repository.
                    pr.head = None;
                    pr
                })
                .ok_or_else(|| GbError::Other(format!("Pull request #{number} not found"))),
        }
    }

    /// Create a pull request
    pub async fn create_pull_request(
        &self,
        owner: &str,
        repo: &str,
        body: &CreatePullRequest,
    ) -> Result<PullRequest> {
        self.request_json(
            reqwest::Method::POST,
            &format!("/repos/{owner}/{repo}/pulls"),
            Some(body),
            super::client::JsonContract::PullRequestCreate,
        )
        .await
    }

    /// Edit implemented PR fields; assignees are a separate operation.
    pub async fn update_pull_request(
        &self,
        owner: &str,
        repo: &str,
        number: u64,
        body: &UpdatePullRequest,
    ) -> Result<PullRequest> {
        self.patch(&format!("/repos/{owner}/{repo}/pulls/{number}"), body)
            .await
    }

    /// Merge a pull request
    pub async fn merge_pull_request(
        &self,
        owner: &str,
        repo: &str,
        number: u64,
        body: &MergePullRequest,
    ) -> Result<MergeResult> {
        self.request_json(
            reqwest::Method::PUT,
            &format!("/repos/{owner}/{repo}/pulls/{number}/merge"),
            Some(body),
            super::client::JsonContract::PullRequestMerge,
        )
        .await
    }

    /// List comments on a pull request (uses issues API)
    pub async fn list_pr_comments(
        &self,
        owner: &str,
        repo: &str,
        number: u64,
    ) -> Result<Vec<Comment>> {
        self.list_issue_comments(owner, repo, number).await
    }

    /// Add a comment to a pull request (uses issues API)
    pub async fn create_pr_comment(
        &self,
        owner: &str,
        repo: &str,
        number: u64,
        body: &CreateComment,
    ) -> Result<Comment> {
        self.create_issue_comment(owner, repo, number, body).await
    }
}
