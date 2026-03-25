use crate::api::client::ApiClient;
use crate::error::{GbError, Result};
use crate::models::comment::{Comment, CreateComment};
use crate::models::pull_request::{CreatePullRequest, MergePullRequest, MergeResult, PullRequest};

impl ApiClient {
    /// List pull requests for a repository
    pub async fn list_pull_requests(
        &self,
        owner: &str,
        repo: &str,
        state: &str,
    ) -> Result<Vec<PullRequest>> {
        self.get(&format!("/repos/{owner}/{repo}/pulls?state={state}"))
            .await
    }

    /// Get a single pull request
    pub async fn get_pull_request(
        &self,
        owner: &str,
        repo: &str,
        number: u64,
    ) -> Result<PullRequest> {
        match self
            .get(&format!("/repos/{}/{}/pulls/{}", owner, repo, number))
            .await
        {
            Ok(pr) => Ok(pr),
            Err(GbError::Json(_)) => {
                let prs = self.list_pull_requests(owner, repo, "all").await?;
                prs.into_iter()
                    .find(|pr| pr.number == number)
                    .ok_or_else(|| GbError::Other(format!("Pull request #{} not found", number)))
            }
            Err(err) => Err(err),
        }
    }

    /// Create a pull request
    pub async fn create_pull_request(
        &self,
        owner: &str,
        repo: &str,
        body: &CreatePullRequest,
    ) -> Result<PullRequest> {
        self.post(&format!("/repos/{}/{}/pulls", owner, repo), body)
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
        self.put(
            &format!("/repos/{}/{}/pulls/{}/merge", owner, repo, number),
            body,
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
        self.get(&format!(
            "/repos/{}/{}/issues/{}/comments",
            owner, repo, number
        ))
        .await
    }

    /// Add a comment to a pull request (uses issues API)
    pub async fn create_pr_comment(
        &self,
        owner: &str,
        repo: &str,
        number: u64,
        body: &CreateComment,
    ) -> Result<Comment> {
        self.post(
            &format!("/repos/{}/{}/issues/{}/comments", owner, repo, number),
            body,
        )
        .await
    }
}
