use dialoguer::Input;

use crate::cli::common::{create_client, resolve_hostname, resolve_repo};
use crate::error::{GbError, Result};
use crate::models::comment::CreateComment;
use crate::models::issue::UpdateIssue;
use crate::models::pull_request::{CreatePullRequest, MergePullRequest};

use super::git::current_branch_name;

pub(super) async fn create(
    hostname: &Option<String>,
    cli_repo: &Option<String>,
    title: Option<String>,
    body: Option<String>,
    head: Option<String>,
    base: Option<String>,
) -> Result<()> {
    let hostname = resolve_hostname(hostname)?;
    let (owner, repo) = resolve_repo(cli_repo)?;
    let client = create_client(&hostname)?;

    let head = match head {
        Some(h) => h,
        None => current_branch_name().ok_or_else(|| {
            GbError::Other(
                "Could not determine current branch. Specify --head when running from a detached HEAD state.".into(),
            )
        })?,
    };

    let base = match base {
        Some(b) => b,
        None => Input::new()
            .with_prompt("Base branch")
            .default("main".to_string())
            .interact_text()?,
    };

    let title = match title {
        Some(t) => t,
        None => Input::new().with_prompt("Title").interact_text()?,
    };

    let body_text = match body {
        Some(b) => Some(b),
        None => {
            let b: String = Input::new()
                .with_prompt("Body (optional)")
                .allow_empty(true)
                .interact_text()?;
            if b.is_empty() {
                None
            } else {
                Some(b)
            }
        }
    };

    let create_body = CreatePullRequest {
        title,
        head,
        base,
        body: body_text,
    };

    let pr = client
        .create_pull_request(&owner, &repo, &create_body)
        .await?;
    println!("✓ Created pull request #{}: {}", pr.number, pr.title);
    if let Some(url) = &pr.html_url {
        println!("{}", url);
    }
    Ok(())
}

pub(super) async fn close(
    hostname: &Option<String>,
    cli_repo: &Option<String>,
    number: u64,
) -> Result<()> {
    let hostname = resolve_hostname(hostname)?;
    let (owner, repo) = resolve_repo(cli_repo)?;
    let client = create_client(&hostname)?;

    let body = UpdateIssue {
        state: Some("closed".to_string()),
        title: None,
        body: None,
    };
    client.update_issue(&owner, &repo, number, &body).await?;
    println!("✓ Closed pull request #{}", number);
    Ok(())
}

pub(super) async fn merge(
    hostname: &Option<String>,
    cli_repo: &Option<String>,
    number: u64,
    message: Option<String>,
) -> Result<()> {
    let hostname = resolve_hostname(hostname)?;
    let (owner, repo) = resolve_repo(cli_repo)?;
    let client = create_client(&hostname)?;

    let body = MergePullRequest {
        commit_message: message,
        sha: None,
        merge_method: None,
    };

    let result = client
        .merge_pull_request(&owner, &repo, number, &body)
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

pub(super) async fn comment(
    hostname: &Option<String>,
    cli_repo: &Option<String>,
    number: u64,
    body: Option<String>,
) -> Result<()> {
    let hostname = resolve_hostname(hostname)?;
    let (owner, repo) = resolve_repo(cli_repo)?;
    let client = create_client(&hostname)?;

    let body_text = match body {
        Some(b) => b,
        None => Input::new().with_prompt("Comment body").interact_text()?,
    };

    let comment_body = CreateComment { body: body_text };
    client
        .create_pr_comment(&owner, &repo, number, &comment_body)
        .await?;
    println!("✓ Added comment to PR #{}", number);
    Ok(())
}
