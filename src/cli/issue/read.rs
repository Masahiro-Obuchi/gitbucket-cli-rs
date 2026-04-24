use colored::Colorize;

use crate::cli::common::{create_client, resolve_hostname, resolve_repo};
use crate::error::{GbError, Result};
use crate::output::table::print_table;
use crate::output::{format_state, truncate};

pub(super) async fn list(
    hostname: &Option<String>,
    cli_repo: &Option<String>,
    cli_profile: &Option<String>,
    state: &str,
    json: bool,
) -> Result<()> {
    let hostname = resolve_hostname(hostname, cli_profile)?;
    let (owner, repo) = resolve_repo(cli_repo, cli_profile)?;
    let client = create_client(&hostname, cli_profile)?;
    let state = crate::cli::common::normalize_list_state(state)?;

    let issues = client.list_issues(&owner, &repo, &state).await?;

    if json {
        println!("{}", serde_json::to_string_pretty(&issues)?);
        return Ok(());
    }

    let rows: Vec<Vec<String>> = issues
        .iter()
        .map(|issue| {
            let labels = issue
                .labels
                .iter()
                .map(|label| label.name.clone())
                .collect::<Vec<_>>()
                .join(", ");
            vec![
                format!("#{}", issue.number),
                format_state(&issue.state),
                truncate(&issue.title, 60),
                issue
                    .user
                    .as_ref()
                    .map(|user| user.login.clone())
                    .unwrap_or_default(),
                labels,
            ]
        })
        .collect();

    print_table(&["#", "STATE", "TITLE", "AUTHOR", "LABELS"], &rows);
    Ok(())
}

pub(super) async fn view(
    hostname: &Option<String>,
    cli_repo: &Option<String>,
    cli_profile: &Option<String>,
    number: u64,
    show_comments: bool,
    web: bool,
    json: bool,
) -> Result<()> {
    let hostname = resolve_hostname(hostname, cli_profile)?;
    let (owner, repo) = resolve_repo(cli_repo, cli_profile)?;
    let client = create_client(&hostname, cli_profile)?;

    if web {
        let url = client.web_url(&format!("/{}/{}/issues/{}", owner, repo, number));
        open::that(&url)
            .map_err(|err| GbError::Other(format!("Failed to open browser: {}", err)))?;
        println!("Opening {} in your browser.", url);
        return Ok(());
    }

    let issue = client.get_issue(&owner, &repo, number).await?;

    if json {
        println!("{}", serde_json::to_string_pretty(&issue)?);
        return Ok(());
    }

    println!(
        "{} {}",
        issue.title.bold(),
        format!("#{}", issue.number).dimmed()
    );
    println!("{}", format_state(&issue.state));
    println!();

    if let Some(user) = &issue.user {
        print!("Author: {}  ", user.login);
    }
    if let Some(created) = &issue.created_at {
        print!("Created: {}", created);
    }
    println!();

    if !issue.labels.is_empty() {
        let labels: Vec<&str> = issue
            .labels
            .iter()
            .map(|label| label.name.as_str())
            .collect();
        println!("Labels: {}", labels.join(", "));
    }
    if !issue.assignees.is_empty() {
        let assignees: Vec<&str> = issue
            .assignees
            .iter()
            .map(|assignee| assignee.login.as_str())
            .collect();
        println!("Assignees: {}", assignees.join(", "));
    }
    if let Some(milestone) = issue.milestone.as_ref() {
        println!("Milestone: {} (#{})", milestone.title, milestone.number);
    }

    if let Some(body) = issue.body.as_deref().filter(|body| !body.is_empty()) {
        println!("\n{}", body);
    }

    if show_comments {
        let comments = client.list_issue_comments(&owner, &repo, number).await?;
        if !comments.is_empty() {
            println!("\n{}", "--- Comments ---".dimmed());
            for comment in &comments {
                let author = comment
                    .user
                    .as_ref()
                    .map(|user| user.login.as_str())
                    .unwrap_or("unknown");
                let date = comment.created_at.as_deref().unwrap_or("");
                println!("\n{} ({})", author.bold(), date.dimmed());
                if let Some(body) = &comment.body {
                    println!("{}", body);
                }
            }
        }
    }

    Ok(())
}
