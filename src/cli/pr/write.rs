use dialoguer::Input;

use crate::cli::common::{create_client, resolve_hostname, resolve_repo};
use crate::error::{GbError, Result};
use crate::models::comment::CreateComment;
use crate::models::issue::UpdateIssue;
use crate::models::pull_request::{CreatePullRequest, MergePullRequest, PullRequest};

use super::git::current_branch_name;
use super::read::print_pr_refs;

#[allow(clippy::too_many_arguments)]
pub(super) async fn create(
    hostname: &Option<String>,
    cli_repo: &Option<String>,
    title: Option<String>,
    body: Option<String>,
    head: Option<String>,
    head_owner: Option<String>,
    base: Option<String>,
    json: bool,
) -> Result<()> {
    let hostname = resolve_hostname(hostname)?;
    let (owner, repo) = resolve_repo(cli_repo)?;
    let client = create_client(&hostname)?;

    let head_branch = match head {
        Some(h) => h,
        None => current_branch_name().ok_or_else(|| {
            GbError::Other(
                "Could not determine current branch. Specify --head when running from a detached HEAD state.".into(),
            )
        })?,
    };
    let head = qualified_head(head_branch, head_owner)?;

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

    if json {
        println!("{}", serde_json::to_string_pretty(&pr)?);
        return Ok(());
    }

    println!("✓ Created pull request #{}: {}", pr.number, pr.title);
    print_pr_refs(&pr);
    println!("URL: {}", pr_url(&client, &owner, &repo, &pr));
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(super) async fn edit(
    hostname: &Option<String>,
    cli_repo: &Option<String>,
    number: u64,
    title: Option<String>,
    body: Option<String>,
    add_assignees: Vec<String>,
    remove_assignees: Vec<String>,
    state: Option<String>,
) -> Result<()> {
    if title.is_none()
        && body.is_none()
        && add_assignees.is_empty()
        && remove_assignees.is_empty()
        && state.is_none()
    {
        return Err(GbError::Other(
            "No pull request changes requested. Pass at least one edit option.".into(),
        ));
    }

    let state = normalize_edit_state(state)?;
    let hostname = resolve_hostname(hostname)?;
    let (owner, repo) = resolve_repo(cli_repo)?;
    let client = create_client(&hostname)?;

    client.get_pull_request(&owner, &repo, number).await?;

    let assignees = if add_assignees.is_empty() && remove_assignees.is_empty() {
        None
    } else {
        let current = client.get_issue(&owner, &repo, number).await?;
        Some(merge_named_values(
            current
                .assignees
                .iter()
                .map(|assignee| assignee.login.clone()),
            add_assignees,
            remove_assignees,
        ))
    };

    let update_body = UpdateIssue {
        state,
        title,
        body,
        labels: None,
        assignees,
        milestone: None,
    };

    let issue = client
        .update_issue(&owner, &repo, number, &update_body)
        .await?;
    println!("✓ Updated pull request #{}: {}", issue.number, issue.title);
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
        labels: None,
        assignees: None,
        milestone: None,
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
                    "No comments by {} found on PR #{}",
                    user.login, number
                ))
            })?;

        client
            .update_issue_comment(&owner, &repo, comment.id, &comment_body)
            .await?;
        println!("✓ Edited comment {} on PR #{}", comment.id, number);
    } else {
        client
            .create_pr_comment(&owner, &repo, number, &comment_body)
            .await?;
        println!("✓ Added comment to PR #{}", number);
    }
    Ok(())
}

fn qualified_head(head: String, head_owner: Option<String>) -> Result<String> {
    let Some(owner) = head_owner else {
        return Ok(head);
    };
    if head.contains(':') {
        return Err(GbError::Other(
            "Cannot use --head-owner when --head is already qualified as OWNER:BRANCH.".into(),
        ));
    }
    let owner = owner.trim();
    if owner.is_empty() {
        return Err(GbError::Other("--head-owner cannot be empty.".into()));
    }
    if owner.contains(':') {
        return Err(GbError::Other(
            "--head-owner cannot contain ':'. Expected an unqualified owner name.".into(),
        ));
    }
    Ok(format!("{owner}:{head}"))
}

fn normalize_edit_state(state: Option<String>) -> Result<Option<String>> {
    match state {
        None => Ok(None),
        Some(value) => match value.to_ascii_lowercase().as_str() {
            "open" | "closed" => Ok(Some(value.to_ascii_lowercase())),
            _ => Err(GbError::Other(
                "Invalid pull request state. Expected 'open' or 'closed'.".into(),
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

fn pr_url(
    client: &crate::api::client::ApiClient,
    owner: &str,
    repo: &str,
    pr: &PullRequest,
) -> String {
    pr.html_url
        .as_deref()
        .filter(|url| !url.contains("/pulls/"))
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| client.web_url(&format!("/{owner}/{repo}/pull/{}", pr.number)))
}

#[cfg(test)]
mod tests {
    use super::qualified_head;

    #[test]
    fn qualified_head_uses_owner_prefix() {
        assert_eq!(
            qualified_head("feature".into(), Some("alice".into())).unwrap(),
            "alice:feature"
        );
    }

    #[test]
    fn qualified_head_rejects_duplicate_owner_syntax() {
        assert!(qualified_head("alice:feature".into(), Some("bob".into())).is_err());
    }
}
