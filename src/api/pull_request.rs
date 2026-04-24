use crate::api::client::ApiClient;
use std::collections::HashSet;

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

    /// List pull requests and fill known gaps in GitBucket's repository PR listing.
    ///
    /// Some GitBucket versions can omit repository-visible open PRs from the
    /// pulls collection while still exposing them through the issues collection.
    pub async fn list_repository_pull_requests(
        &self,
        owner: &str,
        repo: &str,
        state: &str,
    ) -> Result<Vec<PullRequest>> {
        let mut prs = self.list_pull_requests(owner, repo, state).await?;

        if state != "open" {
            return Ok(prs);
        }

        let issues = match self.list_issues(owner, repo, state).await {
            Ok(issues) => issues,
            Err(GbError::Api { status, .. }) if status == 404 || status == 501 => return Ok(prs),
            Err(err) => return Err(err),
        };
        let mut seen: HashSet<u64> = prs.iter().map(|pr| pr.number).collect();

        for issue in issues {
            if issue.pull_request.is_none() || !seen.insert(issue.number) {
                continue;
            }

            match self.get_pull_request(owner, repo, issue.number).await {
                Ok(pr) => prs.push(pr),
                Err(GbError::Api { status, .. }) if status == 404 || status == 501 => continue,
                Err(err) => return Err(err),
            }
        }

        Ok(prs)
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
