//! Validate numbered PR targets through the canonical pulls endpoint before any
//! Issue-backed subresource (comments/assignees) is used.

use crate::cli::common::RepoContext;
use crate::error::{GbError, Result};
use crate::models::pull_request::PullRequest;

pub(super) async fn require_pull_request(ctx: &RepoContext, number: u64) -> Result<PullRequest> {
    // Retain the client's compatibility lookup for null single-PR responses.
    // That fallback searches only the PR collection, never ordinary issues.
    let pr = ctx
        .client
        .get_pull_request(&ctx.owner, &ctx.repo, number)
        .await?;
    if pr.number != number {
        return Err(GbError::Other(format!("PR target mismatch: requested #{number}, API returned #{}. No operation was performed.", pr.number)));
    }
    Ok(pr)
}
