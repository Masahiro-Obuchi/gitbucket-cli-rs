//! Validate numbered PR targets through the canonical pulls endpoint before any
//! Issue-backed subresource (comments/assignees) is used.

use crate::cli::common::RepoContext;
use crate::error::{GbError, Result};
use crate::models::pull_request::PullRequest;

pub(super) async fn require_pull_request(ctx: &RepoContext, number: u64) -> Result<PullRequest> {
    let pr = ctx
        .client
        .pull_request_details(&ctx.owner, &ctx.repo, number)
        .await?
        .ok_or_else(|| {
            GbError::Other(format!(
                "Canonical details for pull request #{number} are unavailable."
            ))
        })?;
    if pr.number != number {
        return Err(GbError::Other(format!("PR target mismatch: requested #{number}, API returned #{}. No operation was performed.", pr.number)));
    }
    Ok(pr)
}
