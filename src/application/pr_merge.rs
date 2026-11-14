//! Client checks and readback are useful evidence, never an atomic merge guard.
use crate::{
    api::client::ApiClient,
    domain::pull_request::CommitId,
    error::{GbError, Result},
    models::pull_request::{MergePullRequest, MergeResult, PullRequest},
    operation::{Progress, Route},
};
use serde::Serialize;

#[derive(Serialize)]
pub(crate) struct MergeReport {
    pub number: u64,
    pub head_commit: String,
    pub atomic: bool,
    pub verification: &'static str,
    pub result: MergeResult,
}

fn check(cause: &'static str, message: impl Into<String>) -> GbError {
    GbError::MergeCheck {
        cause,
        message: message.into(),
    }
}
fn target(pr: &PullRequest, owner: &str, repo: &str, number: u64) -> Result<CommitId> {
    if pr.number != number
        || pr
            .base
            .as_ref()
            .and_then(|b| b.repo.as_ref())
            .is_some_and(|r| r.full_name != format!("{owner}/{repo}"))
    {
        return Err(check(
            "target_mismatch",
            "PR target mismatch: the canonical response belongs to another PR or base repository.",
        ));
    }
    pr.head
        .as_ref()
        .and_then(|h| h.sha.as_deref())
        .and_then(|sha| CommitId::parse(sha).ok())
        .ok_or_else(|| {
            check(
                "missing_head",
                "The canonical PR has no complete saved head SHA; no merge was attempted.",
            )
        })
}
fn ready(pr: &PullRequest) -> Result<()> {
    if pr.state != "open" || pr.merged == Some(true) {
        return Err(check(
            "not_open",
            "The PR is not open and unmerged; no merge was attempted.",
        ));
    }
    if pr.draft == Some(true) {
        return Err(check(
            "draft",
            "Draft PRs cannot be merged; mark the PR ready in GitBucket first.",
        ));
    }
    if pr.mergeable == Some(false) {
        return Err(check(
            "not_mergeable",
            "The server reports that the PR is not mergeable; no merge was attempted.",
        ));
    }
    // Missing/null readiness fields stay unknown; only the server can decide.
    Ok(())
}

pub(crate) async fn merge(
    client: &ApiClient,
    owner: &str,
    repo: &str,
    number: u64,
    expected: Option<CommitId>,
    message: Option<String>,
) -> Result<MergeReport> {
    let mut progress = Progress::default();
    let step = progress.plan("pr.merge", Route::Rest);
    // Compatibility lookup removes unverified summary heads, so it cannot
    // authorize a merge when canonical details are null/unavailable.
    let first = client
        .get_pull_request(owner, repo, number)
        .await
        .map_err(|e| progress.error(e))?;
    let head = target(&first, owner, repo, number).map_err(|e| progress.error(e))?;
    ready(&first).map_err(|e| progress.error(e))?;
    let expected = expected.unwrap_or_else(|| head.clone());
    if head != expected {
        return Err(progress.error(check("head_mismatch", "The PR head differs from --match-head-commit. Review the new head before merging; no merge was attempted.")));
    }
    // Re-read immediately before PUT. This narrows a race; it does not close it.
    let fresh = client
        .pull_request_details(owner, repo, number)
        .await
        .map_err(|e| progress.error(e))?
        .ok_or_else(|| {
            progress.error(check(
                "missing_head",
                "Fresh canonical PR details are unavailable; no merge was attempted.",
            ))
        })?;
    let fresh_head = target(&fresh, owner, repo, number).map_err(|e| progress.error(e))?;
    ready(&fresh).map_err(|e| progress.error(e))?;
    if fresh_head != expected {
        return Err(progress.error(check("head_mismatch", "The PR head changed during merge preparation; no merge was attempted. Review the new head.")));
    }
    // 4.44.0 ignores sha; sending it would imply a server guard we do not have.
    let body = MergePullRequest {
        commit_message: message,
        merge_method: None,
    };
    progress
        .run(
            step,
            client.merge_pull_request(owner, repo, number, &body),
            |result| verify(client, owner, repo, number, &expected, result),
        )
        .await
}

async fn verify(
    client: &ApiClient,
    owner: &str,
    repo: &str,
    number: u64,
    expected: &CommitId,
    result: MergeResult,
) -> Result<MergeReport> {
    // 4.44 serializes the result's JGit ObjectId with toString, not a
    // guaranteed bare hash. Preserve that opaque value; saved head SHAs
    // below are the exact identities we can validate.
    if result.merged != Some(true)
        || result
            .sha
            .as_deref()
            .is_none_or(|sha| sha.trim().is_empty())
    {
        return Err(check(
            "not_confirmed",
            "The merge response did not confirm a merge with a result identity.",
        ));
    }
    let stored = client
        .pull_request_details(owner, repo, number)
        .await?
        .ok_or_else(|| {
            check(
                "readback_mismatch",
                "Canonical PR details are unavailable after the merge response.",
            )
        })?;
    let stored_head = target(&stored, owner, repo, number).map_err(|_| {
        check(
            "readback_mismatch",
            "The merged PR's target or saved head could not be verified.",
        )
    })?;
    if stored.merged != Some(true) || stored.state != "closed" || &stored_head != expected {
        return Err(check("readback_mismatch", "The stored PR does not confirm a closed, merged PR with the checked head. The merge may already have been applied."));
    }
    Ok(MergeReport {
        number,
        head_commit: expected.as_str().to_owned(),
        atomic: false,
        verification: "client_preflight_and_readback",
        result,
    })
}
