use dialoguer::Input;

use crate::cli::common::{
    create_client, create_web_session, merge_named_values, normalize_edit_state, resolve_hostname,
    resolve_repo, update_issue_assignees_via_web,
};
use crate::error::{GbError, Result};
use crate::models::comment::{Comment, CreateComment};
use crate::models::issue::UpdateIssue;
use crate::models::pull_request::{CreatePullRequest, MergePullRequest, PullRequest};

use super::git::current_branch_name;
use super::read::print_pr_refs;

#[allow(clippy::too_many_arguments)]
pub(super) async fn create(
    hostname: &Option<String>,
    cli_repo: &Option<String>,
    cli_profile: &Option<String>,
    title: Option<String>,
    body: Option<String>,
    head: Option<String>,
    head_owner: Option<String>,
    base: Option<String>,
    json: bool,
    detect_existing: bool,
) -> Result<()> {
    let hostname = resolve_hostname(hostname, cli_profile)?;
    let (owner, repo) = resolve_repo(cli_repo, cli_profile)?;
    let client = create_client(&hostname, cli_profile)?;

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

    if detect_existing {
        if let Some(pr) =
            find_existing_open_pull_request(&client, &owner, &repo, &head, &base).await?
        {
            print_pr_create_result(&client, &owner, &repo, &pr, json, "Found existing")?;
            return Ok(());
        }
    }

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
        head: head.clone(),
        base: base.clone(),
        body: body_text,
    };

    match client
        .create_pull_request(&owner, &repo, &create_body)
        .await
    {
        Ok(pr) => print_pr_create_result(&client, &owner, &repo, &pr, json, "Created"),
        Err(err) if detect_existing => {
            match find_existing_open_pull_request(&client, &owner, &repo, &head, &base).await {
                Ok(Some(pr)) => {
                    eprintln!("Notice: PR create failed; returning an existing open PR.");
                    print_pr_create_result(&client, &owner, &repo, &pr, json, "Found existing")
                }
                Ok(None) | Err(_) => Err(err),
            }
        }
        Err(err) => Err(err),
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) async fn edit(
    hostname: &Option<String>,
    cli_repo: &Option<String>,
    cli_profile: &Option<String>,
    number: u64,
    title: Option<String>,
    body: Option<String>,
    add_assignees: Vec<String>,
    remove_assignees: Vec<String>,
    state: Option<String>,
    web: bool,
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

    let state = normalize_edit_state("pull request", state)?;
    let hostname = resolve_hostname(hostname, cli_profile)?;
    let (owner, repo) = resolve_repo(cli_repo, cli_profile)?;
    let client = create_client(&hostname, cli_profile)?;

    let current_pr = client.get_pull_request(&owner, &repo, number).await?;

    let current_issue = if add_assignees.is_empty() && remove_assignees.is_empty() {
        None
    } else {
        Some(client.get_issue(&owner, &repo, number).await?)
    };

    let assignees = current_issue.as_ref().map(|current| {
        merge_named_values(
            current
                .assignees
                .iter()
                .map(|assignee| assignee.login.clone()),
            add_assignees,
            remove_assignees,
        )
    });

    let update_body = UpdateIssue {
        state,
        title,
        body,
        labels: None,
        assignees,
        milestone: None,
    };

    match client
        .update_issue(&owner, &repo, number, &update_body)
        .await
    {
        Ok(issue) => {
            println!("✓ Updated pull request #{}: {}", issue.number, issue.title);
            Ok(())
        }
        Err(GbError::Api { status: 404, .. }) => {
            if !web {
                return Err(GbError::Other(
                    "REST PR edit is unavailable on this GitBucket instance. Re-run with --web to allow the GitBucket web UI fallback.".into(),
                ));
            }

            eprintln!(
                "Notice: REST PR edit is unavailable on this GitBucket instance; using web fallback."
            );
            let session = create_web_session(&hostname, cli_profile).await?;

            let next_title = update_body
                .title
                .clone()
                .unwrap_or_else(|| current_pr.title.clone());
            let next_body = update_body
                .body
                .clone()
                .unwrap_or_else(|| current_pr.body.clone().unwrap_or_default());

            if next_title != current_pr.title {
                session
                    .edit_issue_title(&owner, &repo, number, &next_title)
                    .await?;
            }

            if next_body != current_pr.body.clone().unwrap_or_default() {
                session
                    .edit_issue_content(&owner, &repo, number, &next_title, &next_body)
                    .await?;
            }

            if let (Some(current), Some(next)) =
                (current_issue.as_ref(), update_body.assignees.as_ref())
            {
                update_issue_assignees_via_web(&session, &owner, &repo, number, current, next)
                    .await?;
            }

            if let Some(state) = update_body.state.as_deref() {
                if state != current_pr.state {
                    let action = if state == "closed" { "close" } else { "reopen" };
                    session
                        .update_issue_state(&owner, &repo, number, action)
                        .await?;
                }
            }

            match client.get_pull_request(&owner, &repo, number).await {
                Ok(pr) => {
                    println!("✓ Updated pull request #{}: {}", pr.number, pr.title);
                }
                Err(err) => {
                    eprintln!(
                        "Warning: failed to fetch updated pull request #{} from API after web fallback: {}",
                        number, err
                    );
                    println!("✓ Updated pull request #{}: {}", number, next_title);
                }
            }
            Ok(())
        }
        Err(err) => Err(err),
    }
}

pub(super) async fn close(
    hostname: &Option<String>,
    cli_repo: &Option<String>,
    cli_profile: &Option<String>,
    number: u64,
) -> Result<()> {
    let hostname = resolve_hostname(hostname, cli_profile)?;
    let (owner, repo) = resolve_repo(cli_repo, cli_profile)?;
    let client = create_client(&hostname, cli_profile)?;

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
    cli_profile: &Option<String>,
    number: u64,
    message: Option<String>,
) -> Result<()> {
    let hostname = resolve_hostname(hostname, cli_profile)?;
    let (owner, repo) = resolve_repo(cli_repo, cli_profile)?;
    let client = create_client(&hostname, cli_profile)?;

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
    cli_profile: &Option<String>,
    number: u64,
    body: Option<String>,
    edit_last: bool,
    json: bool,
) -> Result<()> {
    let hostname = resolve_hostname(hostname, cli_profile)?;
    let (owner, repo) = resolve_repo(cli_repo, cli_profile)?;
    let client = create_client(&hostname, cli_profile)?;

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

        let comment = client
            .update_issue_comment(&owner, &repo, comment.id, &comment_body)
            .await?;
        print_comment_result(&comment, number, true, json)?;
    } else {
        let comment = client
            .create_pr_comment(&owner, &repo, number, &comment_body)
            .await?;
        print_comment_result(&comment, number, false, json)?;
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

async fn find_existing_open_pull_request(
    client: &crate::api::client::ApiClient,
    owner: &str,
    repo: &str,
    head: &str,
    base: &str,
) -> Result<Option<PullRequest>> {
    let prs = client.list_pull_requests(owner, repo, "open").await?;
    Ok(prs
        .into_iter()
        .find(|pr| pull_request_matches_head_base(pr, owner, repo, head, base)))
}

fn pull_request_matches_head_base(
    pr: &PullRequest,
    owner: &str,
    repo: &str,
    head: &str,
    base: &str,
) -> bool {
    if pr
        .base
        .as_ref()
        .is_none_or(|pr_base| pr_base.ref_name != base)
    {
        return false;
    }

    let Some(pr_head) = pr.head.as_ref() else {
        return false;
    };

    let (head_owner, head_branch) = head
        .split_once(':')
        .map(|(owner, branch)| (Some(owner), branch))
        .unwrap_or((None, head));
    if pr_head.ref_name != head_branch {
        return false;
    }

    match head_owner {
        Some(head_owner) => {
            pr_head.label.as_deref() == Some(head)
                || pr_head
                    .repo
                    .as_ref()
                    .is_some_and(|head_repo| head_repo.full_name == format!("{head_owner}/{repo}"))
        }
        None => {
            pr_head.label.as_deref() == Some(head)
                || pr_head.label.as_deref() == Some(&format!("{owner}:{head_branch}"))
                || pr_head
                    .repo
                    .as_ref()
                    .is_some_and(|head_repo| head_repo.full_name == format!("{owner}/{repo}"))
        }
    }
}

fn print_pr_create_result(
    client: &crate::api::client::ApiClient,
    owner: &str,
    repo: &str,
    pr: &PullRequest,
    json: bool,
    verb: &str,
) -> Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(pr)?);
        return Ok(());
    }

    println!("✓ {} pull request #{}: {}", verb, pr.number, pr.title);
    print_pr_refs(pr);
    println!("URL: {}", pr_url(client, owner, repo, pr));
    Ok(())
}

fn print_comment_result(comment: &Comment, pr_number: u64, edited: bool, json: bool) -> Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(comment)?);
        return Ok(());
    }

    let action = if edited { "Edited" } else { "Added" };
    println!("✓ {} comment {} on PR #{}", action, comment.id, pr_number);
    if let Some(url) = comment.html_url.as_deref().filter(|url| !url.is_empty()) {
        println!("URL: {}", url);
    }
    Ok(())
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
