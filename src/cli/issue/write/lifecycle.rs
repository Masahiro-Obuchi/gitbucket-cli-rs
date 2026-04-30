use crate::cli::common::{create_web_session, RepoContext};
use crate::error::{GbError, Result};
use crate::models::issue::UpdateIssue;

pub(in crate::cli::issue) async fn close(
    hostname: &Option<String>,
    cli_repo: &Option<String>,
    cli_profile: &Option<String>,
    number: u64,
) -> Result<()> {
    set_issue_state(
        hostname,
        cli_repo,
        cli_profile,
        number,
        "closed",
        "close",
        "Closed",
    )
    .await
}

pub(in crate::cli::issue) async fn reopen(
    hostname: &Option<String>,
    cli_repo: &Option<String>,
    cli_profile: &Option<String>,
    number: u64,
) -> Result<()> {
    set_issue_state(
        hostname,
        cli_repo,
        cli_profile,
        number,
        "open",
        "reopen",
        "Reopened",
    )
    .await
}

async fn set_issue_state(
    hostname: &Option<String>,
    cli_repo: &Option<String>,
    cli_profile: &Option<String>,
    number: u64,
    api_state: &str,
    web_action: &str,
    verb: &str,
) -> Result<()> {
    let ctx = RepoContext::resolve(hostname, cli_repo, cli_profile)?;

    let body = UpdateIssue {
        state: Some(api_state.to_string()),
        title: None,
        body: None,
        labels: None,
        assignees: None,
        milestone: None,
    };

    match ctx
        .client
        .update_issue(&ctx.owner, &ctx.repo, number, &body)
        .await
    {
        Ok(_) => {
            println!("✓ {} issue #{}", verb, number);
            Ok(())
        }
        Err(GbError::Api { status: 404, .. }) => {
            eprintln!(
                "Notice: REST issue state update is unavailable on this GitBucket instance; using web fallback."
            );
            let session = create_web_session(&ctx.hostname, cli_profile).await?;
            session
                .update_issue_state(&ctx.owner, &ctx.repo, number, web_action)
                .await?;
            println!("✓ {} issue #{}", verb, number);
            Ok(())
        }
        Err(err) => Err(err),
    }
}
