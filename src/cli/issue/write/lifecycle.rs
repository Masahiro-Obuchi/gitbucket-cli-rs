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

    let current = ctx.client.get_issue(&ctx.owner, &ctx.repo, number).await?;
    if current.number != number {
        return Err(GbError::Other(
            "Issue lookup returned another number. No changes were applied.".into(),
        ));
    }
    if current.state == api_state {
        println!("✓ {} issue #{}", verb, number);
        return Ok(());
    }
    let body = UpdateIssue {
        state: Some(api_state.to_string()),
        title: None,
        body: None,
        assignees: None,
        milestone: None,
    };
    let mut progress = crate::operation::Progress::default();
    let rest = ctx
        .client
        .update_issue(&ctx.owner, &ctx.repo, number, &body)
        .await;
    match rest {
        Err(GbError::Api { status: 404, .. }) => {
            let step = progress.plan("issue.state", crate::operation::Route::Web);
            let session = create_web_session(&ctx.client, &ctx.hostname, cli_profile)
                .await
                .map_err(|err| progress.error(err))?;
            progress.actor(session.principal());
            progress
                .run(
                    step,
                    session.update_issue_state(&ctx.owner, &ctx.repo, number, web_action),
                    |_| verify_state(&ctx, number, api_state),
                )
                .await?;
        }
        result => {
            let step = progress.plan("issue.state", crate::operation::Route::Rest);
            progress
                .run(step, async { result }, |_| {
                    verify_state(&ctx, number, api_state)
                })
                .await?;
        }
    }
    println!("✓ {} issue #{}", verb, number);
    Ok(())
}

async fn verify_state(ctx: &RepoContext, number: u64, state: &str) -> Result<()> {
    let stored = ctx.client.get_issue(&ctx.owner, &ctx.repo, number).await?;
    if stored.number != number || stored.state != state {
        return Err(GbError::Other(
            "Issue state change was submitted, but stored state does not match.".into(),
        ));
    }
    Ok(())
}
