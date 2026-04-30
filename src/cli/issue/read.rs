use colored::Colorize;

use crate::cli::common::RepoContext;
use crate::error::Result;
use crate::models::issue::Issue;
use crate::output;
use crate::output::table::format_table;
use crate::output::{format_state, page_or_print, truncate};

pub(super) struct ViewOptions {
    pub number: u64,
    pub show_comments: bool,
    pub web: bool,
    pub json: bool,
    pub no_pager: bool,
}

pub(super) async fn list(
    hostname: &Option<String>,
    cli_repo: &Option<String>,
    cli_profile: &Option<String>,
    state: &str,
    json: bool,
    no_pager: bool,
) -> Result<()> {
    let ctx = RepoContext::resolve(hostname, cli_repo, cli_profile)?;
    let state = crate::cli::common::normalize_list_state(state)?;

    let issues = ctx
        .client
        .list_issues(&ctx.owner, &ctx.repo, &state)
        .await?;

    if json {
        return output::page_json(&issues, no_pager);
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

    page_or_print(
        &format_table(&["#", "STATE", "TITLE", "AUTHOR", "LABELS"], &rows),
        no_pager,
    )?;
    Ok(())
}

pub(super) async fn view(
    hostname: &Option<String>,
    cli_repo: &Option<String>,
    cli_profile: &Option<String>,
    options: ViewOptions,
) -> Result<()> {
    let ctx = RepoContext::resolve(hostname, cli_repo, cli_profile)?;

    if options.web {
        let url = ctx.client.web_url(&format!(
            "/{}/{}/issues/{}",
            ctx.owner, ctx.repo, options.number
        ));
        return output::open_web_url(&url);
    }

    let issue = ctx
        .client
        .get_issue(&ctx.owner, &ctx.repo, options.number)
        .await?;

    if options.json {
        return output::page_json(&issue, options.no_pager);
    }

    let mut output = format_issue_view(&issue);
    if options.show_comments {
        let comments = ctx
            .client
            .list_issue_comments(&ctx.owner, &ctx.repo, options.number)
            .await?;
        if !comments.is_empty() {
            output.push_str(&format!("\n{}\n", "--- Comments ---".dimmed()));
            for comment in &comments {
                let author = comment
                    .user
                    .as_ref()
                    .map(|user| user.login.as_str())
                    .unwrap_or("unknown");
                let date = comment.created_at.as_deref().unwrap_or("");
                output.push_str(&format!("\n{} ({})\n", author.bold(), date.dimmed()));
                if let Some(body) = &comment.body {
                    output.push_str(body);
                    output.push('\n');
                }
            }
        }
    }

    page_or_print(&output, options.no_pager)?;
    Ok(())
}

fn format_issue_view(issue: &Issue) -> String {
    let mut output = String::new();
    output.push_str(&format!(
        "{} {}\n",
        issue.title.bold(),
        format!("#{}", issue.number).dimmed()
    ));
    output.push_str(&format!("{}\n\n", format_state(&issue.state)));

    if let Some(user) = &issue.user {
        output.push_str(&format!("Author: {}  ", user.login));
    }
    if let Some(created) = &issue.created_at {
        output.push_str(&format!("Created: {}", created));
    }
    output.push('\n');

    if !issue.labels.is_empty() {
        let labels: Vec<&str> = issue
            .labels
            .iter()
            .map(|label| label.name.as_str())
            .collect();
        output.push_str(&format!("Labels: {}\n", labels.join(", ")));
    }
    if !issue.assignees.is_empty() {
        let assignees: Vec<&str> = issue
            .assignees
            .iter()
            .map(|assignee| assignee.login.as_str())
            .collect();
        output.push_str(&format!("Assignees: {}\n", assignees.join(", ")));
    }
    if let Some(milestone) = issue.milestone.as_ref() {
        output.push_str(&format!(
            "Milestone: {} (#{})\n",
            milestone.title, milestone.number
        ));
    }

    if let Some(body) = issue.body.as_deref().filter(|body| !body.is_empty()) {
        output.push_str(&format!("\n{}\n", body));
    }

    output
}
