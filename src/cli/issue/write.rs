use dialoguer::Input;

use crate::cli::common::{
    create_client, create_web_session, merge_named_values, normalize_edit_state, resolve_hostname,
    resolve_repo, update_issue_assignees_via_web,
};
use crate::error::{GbError, Result};
use crate::models::comment::CreateComment;
use crate::models::issue::{CreateIssue, UpdateIssue};

pub(super) async fn create(
    hostname: &Option<String>,
    cli_repo: &Option<String>,
    cli_profile: &Option<String>,
    title: Option<String>,
    body: Option<String>,
    labels: Vec<String>,
    assignees: Vec<String>,
) -> Result<()> {
    let hostname = resolve_hostname(hostname, cli_profile)?;
    let (owner, repo) = resolve_repo(cli_repo, cli_profile)?;
    let client = create_client(&hostname, cli_profile)?;

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

    let issue = client.create_issue(&owner, &repo, &create_body).await?;
    println!("✓ Created issue #{}: {}", issue.number, issue.title);
    if let Some(url) = &issue.html_url {
        println!("{}", url);
    }
    Ok(())
}

pub(super) async fn close(
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

pub(super) async fn reopen(
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

#[allow(clippy::too_many_arguments)]
pub(super) async fn edit(
    hostname: &Option<String>,
    cli_repo: &Option<String>,
    cli_profile: &Option<String>,
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

    let state = normalize_edit_state("issue", state)?;
    let hostname = resolve_hostname(hostname, cli_profile)?;
    let (owner, repo) = resolve_repo(cli_repo, cli_profile)?;
    let client = create_client(&hostname, cli_profile)?;
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
            if update_body.labels.is_some() {
                return Err(GbError::Other(
                    "This GitBucket instance does not support editing issue labels through the web fallback; the web fallback cannot edit labels. Retry against an instance with REST issue edit support, or update title/body/assignees/milestone/state only.".into(),
                ));
            }

            eprintln!(
                "Notice: REST issue edit is unavailable on this GitBucket instance; using web fallback."
            );
            let session = create_web_session(&hostname, cli_profile).await?;

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

            if let Some(next_assignees) = update_body.assignees.as_ref() {
                update_issue_assignees_via_web(
                    &session,
                    &owner,
                    &repo,
                    number,
                    &current,
                    next_assignees,
                )
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
    cli_profile: &Option<String>,
    number: u64,
    api_state: &str,
    web_action: &str,
    verb: &str,
) -> Result<()> {
    let hostname = resolve_hostname(hostname, cli_profile)?;
    let (owner, repo) = resolve_repo(cli_repo, cli_profile)?;
    let client = create_client(&hostname, cli_profile)?;

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
            eprintln!(
                "Notice: REST issue state update is unavailable on this GitBucket instance; using web fallback."
            );
            let session = create_web_session(&hostname, cli_profile).await?;
            session
                .update_issue_state(&owner, &repo, number, web_action)
                .await?;
            println!("✓ {} issue #{}", verb, number);
            Ok(())
        }
        Err(err) => Err(err),
    }
}

pub(super) async fn comment(
    hostname: &Option<String>,
    cli_repo: &Option<String>,
    cli_profile: &Option<String>,
    number: u64,
    body: Option<String>,
    edit_last: bool,
) -> Result<()> {
    let hostname = resolve_hostname(hostname, cli_profile)?;
    let (owner, repo) = resolve_repo(cli_repo, cli_profile)?;
    let client = create_client(&hostname, cli_profile)?;

    let body_text = match body {
        Some(body) => body,
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
