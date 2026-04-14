use clap::{Args, Subcommand};
use colored::Colorize;
use dialoguer::Input;

use crate::cli::common::{create_client, create_web_session, resolve_hostname, resolve_repo};
use crate::error::{GbError, Result};
use crate::models::comment::CreateComment;
use crate::models::issue::{CreateIssue, UpdateIssue};
use crate::output::table::print_table;
use crate::output::{format_state, truncate};

#[derive(Args)]
pub struct IssueArgs {
    #[command(subcommand)]
    pub command: IssueCommand,
}

#[derive(Subcommand)]
pub enum IssueCommand {
    /// List issues
    List {
        /// Filter by state (open, closed, all)
        #[arg(
            long,
            short,
            default_value = "open",
            value_parser = ["open", "closed", "all"],
            ignore_case = true
        )]
        state: String,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
    /// View an issue (use --comments to include comments)
    View {
        /// Issue number
        number: u64,
        /// Include comments in the output
        #[arg(long, short)]
        comments: bool,
        /// Open in browser
        #[arg(long, short)]
        web: bool,
    },
    /// Create a new issue
    Create {
        /// Issue title (prompts when omitted)
        #[arg(long, short)]
        title: Option<String>,
        /// Issue body (prompts when omitted)
        #[arg(long, short)]
        body: Option<String>,
        /// Label name (repeatable or comma-separated)
        #[arg(long, short, value_delimiter = ',')]
        label: Vec<String>,
        /// Assignee username (repeatable or comma-separated)
        #[arg(long, short, value_delimiter = ',')]
        assignee: Vec<String>,
    },
    /// Edit an issue
    Edit {
        /// Issue number
        number: u64,
        /// New issue title
        #[arg(long, short)]
        title: Option<String>,
        /// New issue body
        #[arg(long, short)]
        body: Option<String>,
        /// Add label name (repeatable or comma-separated)
        #[arg(long = "add-label", value_delimiter = ',')]
        add_label: Vec<String>,
        /// Remove label name (repeatable or comma-separated)
        #[arg(long = "remove-label", value_delimiter = ',')]
        remove_label: Vec<String>,
        /// Add assignee username (repeatable or comma-separated)
        #[arg(long = "add-assignee", value_delimiter = ',')]
        add_assignee: Vec<String>,
        /// Remove assignee username (repeatable or comma-separated)
        #[arg(long = "remove-assignee", value_delimiter = ',')]
        remove_assignee: Vec<String>,
        /// Set milestone number
        #[arg(long)]
        milestone: Option<u64>,
        /// Remove the current milestone
        #[arg(long)]
        remove_milestone: bool,
        /// Update issue state (open or closed)
        #[arg(long, value_parser = ["open", "closed"], ignore_case = true)]
        state: Option<String>,
    },
    /// Close an issue
    Close {
        /// Issue number
        number: u64,
    },
    /// Reopen an issue
    Reopen {
        /// Issue number
        number: u64,
    },
    /// Add or edit a comment on an issue
    Comment {
        /// Issue number
        number: u64,
        /// Comment body (prompts when omitted)
        #[arg(long, short)]
        body: Option<String>,
        /// Edit your last comment instead of adding a new one
        #[arg(long)]
        edit_last: bool,
    },
}

pub async fn run(
    args: IssueArgs,
    cli_hostname: &Option<String>,
    cli_repo: &Option<String>,
) -> Result<()> {
    match args.command {
        IssueCommand::List { state, json } => list(cli_hostname, cli_repo, &state, json).await,
        IssueCommand::View {
            number,
            comments,
            web,
        } => view(cli_hostname, cli_repo, number, comments, web).await,
        IssueCommand::Create {
            title,
            body,
            label,
            assignee,
        } => {
            create(
                cli_hostname,
                cli_repo,
                title,
                body,
                normalize_str_vec(label),
                normalize_str_vec(assignee),
            )
            .await
        }
        IssueCommand::Edit {
            number,
            title,
            body,
            add_label,
            remove_label,
            add_assignee,
            remove_assignee,
            milestone,
            remove_milestone,
            state,
        } => {
            edit(
                cli_hostname,
                cli_repo,
                number,
                title,
                body,
                normalize_str_vec(add_label),
                normalize_str_vec(remove_label),
                normalize_str_vec(add_assignee),
                normalize_str_vec(remove_assignee),
                milestone,
                remove_milestone,
                state,
            )
            .await
        }
        IssueCommand::Close { number } => close(cli_hostname, cli_repo, number).await,
        IssueCommand::Reopen { number } => reopen(cli_hostname, cli_repo, number).await,
        IssueCommand::Comment {
            number,
            body,
            edit_last,
        } => comment(cli_hostname, cli_repo, number, body, edit_last).await,
    }
}

async fn list(
    hostname: &Option<String>,
    cli_repo: &Option<String>,
    state: &str,
    json: bool,
) -> Result<()> {
    let hostname = resolve_hostname(hostname)?;
    let (owner, repo) = resolve_repo(cli_repo)?;
    let client = create_client(&hostname)?;
    let state = crate::cli::common::normalize_list_state(state)?;

    let issues = client.list_issues(&owner, &repo, &state).await?;

    if json {
        println!("{}", serde_json::to_string_pretty(&issues)?);
        return Ok(());
    }

    let rows: Vec<Vec<String>> = issues
        .iter()
        .map(|i| {
            let labels = i
                .labels
                .iter()
                .map(|l| l.name.clone())
                .collect::<Vec<_>>()
                .join(", ");
            vec![
                format!("#{}", i.number),
                format_state(&i.state),
                truncate(&i.title, 60),
                i.user.as_ref().map(|u| u.login.clone()).unwrap_or_default(),
                labels,
            ]
        })
        .collect();

    print_table(&["#", "STATE", "TITLE", "AUTHOR", "LABELS"], &rows);
    Ok(())
}

async fn view(
    hostname: &Option<String>,
    cli_repo: &Option<String>,
    number: u64,
    show_comments: bool,
    web: bool,
) -> Result<()> {
    let hostname = resolve_hostname(hostname)?;
    let (owner, repo) = resolve_repo(cli_repo)?;
    let client = create_client(&hostname)?;

    if web {
        let url = client.web_url(&format!("/{}/{}/issues/{}", owner, repo, number));
        open::that(&url)
            .map_err(|e| crate::error::GbError::Other(format!("Failed to open browser: {}", e)))?;
        println!("Opening {} in your browser.", url);
        return Ok(());
    }

    let issue = client.get_issue(&owner, &repo, number).await?;

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
        let labels: Vec<&str> = issue.labels.iter().map(|l| l.name.as_str()).collect();
        println!("Labels: {}", labels.join(", "));
    }
    if !issue.assignees.is_empty() {
        let assignees: Vec<&str> = issue.assignees.iter().map(|u| u.login.as_str()).collect();
        println!("Assignees: {}", assignees.join(", "));
    }
    if let Some(milestone) = issue.milestone.as_ref() {
        println!("Milestone: {} (#{})", milestone.title, milestone.number);
    }

    if let Some(body) = &issue.body {
        if !body.is_empty() {
            println!(
                "
{}",
                body
            );
        }
    }

    if show_comments {
        let comments = client.list_issue_comments(&owner, &repo, number).await?;
        if !comments.is_empty() {
            println!(
                "
{}",
                "--- Comments ---".dimmed()
            );
            for c in &comments {
                let author = c
                    .user
                    .as_ref()
                    .map(|u| u.login.as_str())
                    .unwrap_or("unknown");
                let date = c.created_at.as_deref().unwrap_or("");
                println!(
                    "
{} ({})",
                    author.bold(),
                    date.dimmed()
                );
                if let Some(body) = &c.body {
                    println!("{}", body);
                }
            }
        }
    }

    Ok(())
}

async fn create(
    hostname: &Option<String>,
    cli_repo: &Option<String>,
    title: Option<String>,
    body: Option<String>,
    labels: Vec<String>,
    assignees: Vec<String>,
) -> Result<()> {
    let hostname = resolve_hostname(hostname)?;
    let (owner, repo) = resolve_repo(cli_repo)?;
    let client = create_client(&hostname)?;

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

    let issue = client.create_issue(&owner, &repo, &create_body).await?;
    println!("✓ Created issue #{}: {}", issue.number, issue.title);
    if let Some(url) = &issue.html_url {
        println!("{}", url);
    }
    Ok(())
}

async fn close(hostname: &Option<String>, cli_repo: &Option<String>, number: u64) -> Result<()> {
    set_issue_state(hostname, cli_repo, number, "closed", "close", "Closed").await
}

async fn reopen(hostname: &Option<String>, cli_repo: &Option<String>, number: u64) -> Result<()> {
    set_issue_state(hostname, cli_repo, number, "open", "reopen", "Reopened").await
}

#[allow(clippy::too_many_arguments)]
async fn edit(
    hostname: &Option<String>,
    cli_repo: &Option<String>,
    number: u64,
    title: Option<String>,
    body: Option<String>,
    add_labels: Vec<String>,
    remove_labels: Vec<String>,
    add_assignees: Vec<String>,
    remove_assignees: Vec<String>,
    milestone: Option<u64>,
    remove_milestone: bool,
    state: Option<String>,
) -> Result<()> {
    if title.is_none()
        && body.is_none()
        && add_labels.is_empty()
        && remove_labels.is_empty()
        && add_assignees.is_empty()
        && remove_assignees.is_empty()
        && milestone.is_none()
        && !remove_milestone
        && state.is_none()
    {
        return Err(GbError::Other(
            "No issue changes requested. Pass at least one edit option.".into(),
        ));
    }

    if milestone.is_some() && remove_milestone {
        return Err(GbError::Other(
            "Cannot use --milestone and --remove-milestone together.".into(),
        ));
    }

    let state = normalize_edit_state(state)?;
    let hostname = resolve_hostname(hostname)?;
    let (owner, repo) = resolve_repo(cli_repo)?;
    let client = create_client(&hostname)?;
    let current = client.get_issue(&owner, &repo, number).await?;

    let labels = if add_labels.is_empty() && remove_labels.is_empty() {
        None
    } else {
        Some(merge_named_values(
            current.labels.iter().map(|label| label.name.clone()),
            add_labels,
            remove_labels,
        ))
    };

    let assignees = if add_assignees.is_empty() && remove_assignees.is_empty() {
        None
    } else {
        Some(merge_named_values(
            current
                .assignees
                .iter()
                .map(|assignee| assignee.login.clone()),
            add_assignees,
            remove_assignees,
        ))
    };

    let milestone = if remove_milestone {
        Some(None)
    } else {
        milestone.map(Some)
    };

    let update_body = UpdateIssue {
        state,
        title,
        body,
        labels,
        assignees,
        milestone,
    };

    match client
        .update_issue(&owner, &repo, number, &update_body)
        .await
    {
        Ok(issue) => {
            println!("✓ Updated issue #{}: {}", issue.number, issue.title);
            Ok(())
        }
        Err(GbError::Api { status: 404, .. }) => {
            if update_body.labels.is_some() || update_body.assignees.is_some() {
                return Err(GbError::Other(
                    "This GitBucket instance does not support editing issue labels or assignees through the web fallback. Retry against an instance with REST issue edit support, or update title/body/milestone/state only.".into(),
                ));
            }

            let session = create_web_session(&hostname).await?;

            let next_title = update_body
                .title
                .clone()
                .unwrap_or_else(|| current.title.clone());
            let next_body = update_body
                .body
                .clone()
                .unwrap_or_else(|| current.body.clone().unwrap_or_default());

            if next_title != current.title {
                session
                    .edit_issue_title(&owner, &repo, number, &next_title)
                    .await?;
            }

            if next_body != current.body.clone().unwrap_or_default() {
                session
                    .edit_issue_content(&owner, &repo, number, &next_title, &next_body)
                    .await?;
            }

            if let Some(milestone) = update_body.milestone {
                session
                    .update_issue_milestone(&owner, &repo, number, milestone)
                    .await?;
            }

            if let Some(state) = update_body.state.as_deref() {
                if state != current.state {
                    let action = if state == "closed" { "close" } else { "reopen" };
                    session
                        .update_issue_state(&owner, &repo, number, action)
                        .await?;
                }
            }

            match client.get_issue(&owner, &repo, number).await {
                Ok(issue) => {
                    println!("✓ Updated issue #{}: {}", issue.number, issue.title);
                }
                Err(err) => {
                    eprintln!(
                        "Warning: failed to fetch updated issue #{} from API after web fallback: {}",
                        number, err
                    );
                    println!("✓ Updated issue #{}: {}", number, next_title);
                }
            }
            Ok(())
        }
        Err(err) => Err(err),
    }
}

async fn set_issue_state(
    hostname: &Option<String>,
    cli_repo: &Option<String>,
    number: u64,
    api_state: &str,
    web_action: &str,
    verb: &str,
) -> Result<()> {
    let hostname = resolve_hostname(hostname)?;
    let (owner, repo) = resolve_repo(cli_repo)?;
    let client = create_client(&hostname)?;

    let body = UpdateIssue {
        state: Some(api_state.to_string()),
        title: None,
        body: None,
        labels: None,
        assignees: None,
        milestone: None,
    };

    match client.update_issue(&owner, &repo, number, &body).await {
        Ok(_) => {
            println!("✓ {} issue #{}", verb, number);
            Ok(())
        }
        Err(GbError::Api { status: 404, .. }) => {
            let session = create_web_session(&hostname).await?;
            session
                .update_issue_state(&owner, &repo, number, web_action)
                .await?;
            println!("✓ {} issue #{}", verb, number);
            Ok(())
        }
        Err(err) => Err(err),
    }
}

async fn comment(
    hostname: &Option<String>,
    cli_repo: &Option<String>,
    number: u64,
    body: Option<String>,
    edit_last: bool,
) -> Result<()> {
    let hostname = resolve_hostname(hostname)?;
    let (owner, repo) = resolve_repo(cli_repo)?;
    let client = create_client(&hostname)?;

    let body_text = match body {
        Some(b) => b,
        None => Input::new().with_prompt("Comment body").interact_text()?,
    };

    let comment_body = CreateComment { body: body_text };
    if edit_last {
        let user = client.current_user().await?;
        let comments = client
            .list_all_issue_comments(&owner, &repo, number)
            .await?;
        let comment = comments
            .iter()
            .filter(|comment| {
                comment
                    .user
                    .as_ref()
                    .is_some_and(|comment_user| comment_user.login == user.login)
            })
            .max_by_key(|comment| comment.id)
            .ok_or_else(|| {
                GbError::Other(format!(
                    "No comments by {} found on issue #{}",
                    user.login, number
                ))
            })?;

        client
            .update_issue_comment(&owner, &repo, comment.id, &comment_body)
            .await?;
        println!("✓ Edited comment {} on issue #{}", comment.id, number);
    } else {
        client
            .create_issue_comment(&owner, &repo, number, &comment_body)
            .await?;
        println!("✓ Added comment to issue #{}", number);
    }
    Ok(())
}

fn normalize_str_vec(values: Vec<String>) -> Vec<String> {
    values
        .into_iter()
        .map(|v| v.trim().to_owned())
        .filter(|s| !s.is_empty())
        .collect()
}

fn normalize_edit_state(state: Option<String>) -> Result<Option<String>> {
    match state {
        None => Ok(None),
        Some(value) => match value.to_ascii_lowercase().as_str() {
            "open" | "closed" => Ok(Some(value.to_ascii_lowercase())),
            _ => Err(GbError::Other(
                "Invalid issue state. Expected 'open' or 'closed'.".into(),
            )),
        },
    }
}

fn merge_named_values(
    current: impl IntoIterator<Item = String>,
    additions: Vec<String>,
    removals: Vec<String>,
) -> Vec<String> {
    let mut values: Vec<String> = current.into_iter().collect();
    values.retain(|value| !removals.iter().any(|removed| removed == value));
    for addition in additions {
        if !values.iter().any(|existing| existing == &addition) {
            values.push(addition);
        }
    }
    values
}

#[cfg(test)]
mod tests {
    use super::{merge_named_values, normalize_edit_state, normalize_str_vec};

    #[test]
    fn normalize_edit_state_accepts_open_and_closed() {
        assert_eq!(
            normalize_edit_state(Some("open".into())).unwrap(),
            Some("open".into())
        );
        assert_eq!(
            normalize_edit_state(Some("closed".into())).unwrap(),
            Some("closed".into())
        );
        assert_eq!(
            normalize_edit_state(Some("OPEN".into())).unwrap(),
            Some("open".into())
        );
        assert_eq!(
            normalize_edit_state(Some("Closed".into())).unwrap(),
            Some("closed".into())
        );
    }

    #[test]
    fn normalize_edit_state_rejects_other_values() {
        assert!(normalize_edit_state(Some("all".into())).is_err());
    }

    #[test]
    fn merge_named_values_applies_removals_then_additions() {
        let merged = merge_named_values(
            vec!["bug".into(), "urgent".into()],
            vec!["enhancement".into(), "urgent".into()],
            vec!["bug".into()],
        );

        assert_eq!(merged, vec!["urgent", "enhancement"]);
    }

    #[test]
    fn normalize_str_vec_trims_whitespace_and_drops_empty() {
        assert_eq!(
            normalize_str_vec(vec!["bug".into(), " urgent".into(), "".into()]),
            vec!["bug", "urgent"]
        );
        assert_eq!(
            normalize_str_vec(vec!["  alice  ".into(), "  ".into(), "bob".into()]),
            vec!["alice", "bob"]
        );
        assert_eq!(
            normalize_str_vec(vec!["".into(), "  ".into()]),
            Vec::<String>::new()
        );
    }
}
