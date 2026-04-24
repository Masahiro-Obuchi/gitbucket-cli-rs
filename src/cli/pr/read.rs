use colored::Colorize;

use crate::cli::common::{create_client, resolve_hostname, resolve_repo};
use crate::error::{GbError, Result};
use crate::models::pull_request::PullRequest;
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

    let prs = client
        .list_repository_pull_requests(&owner, &repo, &state)
        .await?;

    if json {
        println!("{}", serde_json::to_string_pretty(&prs)?);
        return Ok(());
    }

    let rows: Vec<Vec<String>> = prs
        .iter()
        .map(|pr| {
            let state = if pr.merged == Some(true) {
                "merged"
            } else {
                &pr.state
            };
            let branch = pr.head.as_ref().map(|h| h.ref_name.as_str()).unwrap_or("");
            vec![
                format!("#{}", pr.number),
                format_state(state),
                truncate(&pr.title, 50),
                branch.to_string(),
                pr.user
                    .as_ref()
                    .map(|u| u.login.clone())
                    .unwrap_or_default(),
            ]
        })
        .collect();

    print_table(&["#", "STATE", "TITLE", "BRANCH", "AUTHOR"], &rows);
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
        let url = client.web_url(&format!("/{}/{}/pull/{}", owner, repo, number));
        open::that(&url).map_err(|e| GbError::Other(format!("Failed to open browser: {}", e)))?;
        println!("Opening {} in your browser.", url);
        return Ok(());
    }

    let pr = client.get_pull_request(&owner, &repo, number).await?;

    if json {
        println!("{}", serde_json::to_string_pretty(&pr)?);
        return Ok(());
    }

    let state = if pr.merged == Some(true) {
        "merged"
    } else {
        &pr.state
    };

    println!("{} {}", pr.title.bold(), format!("#{}", pr.number).dimmed());
    println!("{}", format_state(state));
    println!();

    if let Some(head) = &pr.head {
        if let Some(base) = &pr.base {
            println!("{} ← {}", base.ref_name.cyan(), head.ref_name.green());
        }
    }

    if let Some(user) = &pr.user {
        print!("Author: {}  ", user.login);
    }
    if let Some(created) = &pr.created_at {
        print!("Created: {}", created);
    }
    println!();

    if let Some(body) = &pr.body {
        if !body.is_empty() {
            println!("\n{}", body);
        }
    }

    if show_comments {
        let comments = client.list_pr_comments(&owner, &repo, number).await?;
        if !comments.is_empty() {
            println!("\n{}", "--- Comments ---".dimmed());
            for c in &comments {
                let author = c
                    .user
                    .as_ref()
                    .map(|u| u.login.as_str())
                    .unwrap_or("unknown");
                let date = c.created_at.as_deref().unwrap_or("");
                println!("\n{} ({})", author.bold(), date.dimmed());
                if let Some(body) = &c.body {
                    println!("{}", body);
                }
            }
        }
    }

    Ok(())
}

pub(super) fn print_pr_refs(pr: &PullRequest) {
    if let Some(head) = &pr.head {
        println!("Head: {}", format_pr_ref(head));
    }
    if let Some(base) = &pr.base {
        println!("Base: {}", format_pr_ref(base));
    }
}

fn format_pr_ref(pr_ref: &crate::models::pull_request::PullRequestHead) -> String {
    match &pr_ref.repo {
        Some(repo) => format!("{}:{}", repo.full_name, pr_ref.ref_name),
        None => pr_ref
            .label
            .clone()
            .unwrap_or_else(|| pr_ref.ref_name.clone()),
    }
}
