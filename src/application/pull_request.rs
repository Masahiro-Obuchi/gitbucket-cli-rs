//! Load saved identities without trusting a collection's fork ownership.
use crate::api::client::ApiClient;
use crate::domain::pull_request::{CommitId, PullRequestSnapshot};
use crate::error::{GbError, Result};
use crate::models::repository::Repository;

pub(crate) struct SavedPullRequest {
    pub snapshot: PullRequestSnapshot,
    pub state: String,
    pub merged: Option<bool>,
    pub base_repository: Option<Repository>,
    pub head_repository: Option<Repository>,
}

pub(crate) async fn load_saved_pull_request(
    client: &ApiClient,
    owner: &str,
    repo: &str,
    number: u64,
) -> Result<SavedPullRequest> {
    let (pr, canonical) = match client
        .pull_request_snapshot_details(owner, repo, number)
        .await?
    {
        Some(pr) => (pr, true),
        None => (
            client
                .list_pull_requests(owner, repo, "all")
                .await?
                .into_iter()
                .find(|pr| pr.number == number)
                .ok_or_else(|| GbError::Other(format!("Pull request #{number} not found")))?,
            false,
        ),
    };
    if pr.number != number {
        return Err(GbError::Other(format!(
            "PR target mismatch: requested #{number}, API returned #{}.",
            pr.number
        )));
    }
    let unavailable = |part: &str| {
        GbError::DiffUnavailable {
        number,
        cause: "missing_snapshot",
        message: format!(
            "PR has no valid saved {part} SHA. A current branch or unverified text diff cannot replace the PR snapshot."
        ),
    }
    };
    let head = pr.head.as_ref().ok_or_else(|| unavailable("head"))?;
    let base = pr.base.as_ref().ok_or_else(|| unavailable("base"))?;
    let head_sha = CommitId::parse(head.sha.as_deref().ok_or_else(|| unavailable("head"))?)
        .map_err(|_| unavailable("head"))?;
    let base_sha = CommitId::parse(base.sha.as_deref().ok_or_else(|| unavailable("base"))?)
        .map_err(|_| unavailable("base"))?;
    if base
        .repo
        .as_ref()
        .is_some_and(|base| base.full_name != format!("{owner}/{repo}"))
    {
        return Err(GbError::Other(
            "PR base repository does not match the selected target.".into(),
        ));
    }
    Ok(SavedPullRequest {
        state: pr.state.clone(),
        merged: pr.merged,
        snapshot: PullRequestSnapshot {
            number,
            base: base_sha,
            head: head_sha,
            base_branch: Some(base.ref_name.clone()),
            head_branch: Some(head.ref_name.clone()),
        },
        base_repository: base.repo.clone(),
        // 4.44's collection contains saved SHAs, but misidentifies fork repos.
        // When detail is null/200-empty (e.g. a deleted fork), only the selected base's
        // retained PR ref/objects may recover the snapshot.
        head_repository: canonical.then(|| head.repo.clone()).flatten(),
    })
}
