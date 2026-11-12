use crate::cli::common::RepoContext;
use crate::error::Result;
use crate::models::issue::CreateIssue;

pub(in crate::cli::issue) async fn create(
    hostname: &Option<String>,
    cli_repo: &Option<String>,
    cli_profile: &Option<String>,
    title: Option<String>,
    body: Option<String>,
    labels: Vec<String>,
    assignees: Vec<String>,
) -> Result<()> {
    let ctx = RepoContext::resolve(hostname, cli_repo, cli_profile)?;

    let title = crate::cli::input::title(title).await?;
    let body_text = crate::cli::input::optional_body(body).await?;

    let create_body = CreateIssue {
        title,
        body: body_text,
        labels: if labels.is_empty() {
            None
        } else {
            Some(labels)
        },
        assignees: if assignees.is_empty() {
            None
        } else {
            Some(assignees)
        },
        milestone: None,
    };

    let issue = ctx
        .client
        .create_issue(&ctx.owner, &ctx.repo, &create_body)
        .await?;
    println!(
        "✓ Created issue #{}: {}",
        issue.number,
        crate::output::single_line(&issue.title)
    );
    if let Some(url) = &issue.html_url {
        println!("{}", url);
    }
    Ok(())
}
