use crate::cli::common::RepoContext;
use crate::error::{GbError, Result};
use crate::models::issue::UpdateIssue;
use crate::models::pull_request::MergePullRequest;

pub(in crate::cli::pr) async fn close(
    hostname: &Option<String>,
    cli_repo: &Option<String>,
    cli_profile: &Option<String>,
    number: u64,
) -> Result<()> {
    let ctx = RepoContext::resolve(hostname, cli_repo, cli_profile)?;

    let body = UpdateIssue {
        state: Some("closed".to_string()),
        title: None,
        body: None,
        labels: None,
        assignees: None,
        milestone: None,
    };
    ctx.client
        .update_issue(&ctx.owner, &ctx.repo, number, &body)
        .await?;
    println!("✓ Closed pull request #{}", number);
    Ok(())
}

pub(in crate::cli::pr) async fn merge(
    hostname: &Option<String>,
    cli_repo: &Option<String>,
    cli_profile: &Option<String>,
    number: u64,
    message: Option<String>,
) -> Result<()> {
    let ctx = RepoContext::resolve(hostname, cli_repo, cli_profile)?;

    let body = MergePullRequest {
        commit_message: message,
        sha: None,
        merge_method: None,
    };

    let result = ctx
        .client
        .merge_pull_request(&ctx.owner, &ctx.repo, number, &body)
        .await?;
    if result.merged == Some(true) {
        println!("✓ Merged pull request #{}", number);
        Ok(())
    } else {
        let msg = result
            .message
            .unwrap_or_else(|| "Unknown error".to_string());
        Err(GbError::Other(format!(
            "Failed to merge pull request #{}: {}",
            number, msg
        )))
    }
}
