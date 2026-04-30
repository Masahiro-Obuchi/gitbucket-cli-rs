use dialoguer::Input;

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

    let title = match title {
        Some(title) => title,
        None => Input::new().with_prompt("Title").interact_text()?,
    };

    let body_text = match body {
        Some(body) => Some(body),
        None => {
            let body: String = Input::new()
                .with_prompt("Body (optional)")
                .allow_empty(true)
                .interact_text()?;
            if body.is_empty() {
                None
            } else {
                Some(body)
            }
        }
    };

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
    println!("✓ Created issue #{}: {}", issue.number, issue.title);
    if let Some(url) = &issue.html_url {
        println!("{}", url);
    }
    Ok(())
}
