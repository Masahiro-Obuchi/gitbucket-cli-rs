use crate::api::client::ApiClient;
use crate::api::web::GitBucketWebSession;
use crate::cli::common::update_issue_assignees_via_web;
use crate::error::{GbError, Result};
use crate::models::comment::Comment;
use crate::models::issue::{Issue, UpdateIssue};

pub(crate) struct IssueLikeEdit<'a> {
    pub current: &'a IssueLikeCurrent,
    pub update: &'a UpdateIssue,
    pub allow_web_fallback: bool,
    pub unsupported_web_fallback_message: Option<&'a str>,
    pub rest_unavailable_notice: &'a str,
    pub success_noun: &'a str,
}

pub(crate) struct IssueLikeTarget<'a> {
    pub session: &'a GitBucketWebSession,
    pub owner: &'a str,
    pub repo: &'a str,
    pub number: u64,
}

pub(crate) struct IssueLikeCurrent {
    pub title: String,
    pub body: Option<String>,
    pub state: String,
    pub issue: Option<Issue>,
}

impl IssueLikeCurrent {
    pub(crate) fn issue(issue: Issue) -> Self {
        Self {
            title: issue.title.clone(),
            body: issue.body.clone(),
            state: issue.state.clone(),
            issue: Some(issue),
        }
    }

    pub(crate) fn pull_request(
        title: String,
        body: Option<String>,
        state: String,
        issue: Option<Issue>,
    ) -> Self {
        Self {
            title,
            body,
            state,
            issue,
        }
    }
}

pub(crate) async fn run_issue_like_edit<F, Fut>(
    target: IssueLikeTarget<'_>,
    edit: IssueLikeEdit<'_>,
    fetch_after: F,
) -> Result<()>
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = Result<(u64, String)>>,
{
    if !edit.allow_web_fallback {
        return Err(GbError::Other(
            edit.unsupported_web_fallback_message
                .unwrap_or("REST edit is unavailable on this GitBucket instance.")
                .into(),
        ));
    }

    if edit.update.labels.is_some() {
        return Err(GbError::Other(
            edit.unsupported_web_fallback_message
                .unwrap_or("This GitBucket instance does not support editing labels through the web fallback.")
                .into(),
        ));
    }

    eprintln!("{}", edit.rest_unavailable_notice);

    let next_title = edit
        .update
        .title
        .clone()
        .unwrap_or_else(|| edit.current.title.clone());
    let current_body = edit.current.body.clone().unwrap_or_default();
    let next_body = edit
        .update
        .body
        .clone()
        .unwrap_or_else(|| current_body.clone());

    if next_title != edit.current.title {
        target
            .session
            .edit_issue_title(target.owner, target.repo, target.number, &next_title)
            .await?;
    }

    if next_body != current_body {
        target
            .session
            .edit_issue_content(
                target.owner,
                target.repo,
                target.number,
                &next_title,
                &next_body,
            )
            .await?;
    }

    if let Some(milestone) = edit.update.milestone {
        target
            .session
            .update_issue_milestone(target.owner, target.repo, target.number, milestone)
            .await?;
    }

    if let Some(next_assignees) = edit.update.assignees.as_ref() {
        let current = edit.current.issue.as_ref().ok_or_else(|| {
            GbError::Other(
                "Cannot update assignees through web fallback without issue details.".into(),
            )
        })?;
        update_issue_assignees_via_web(
            target.session,
            target.owner,
            target.repo,
            target.number,
            current,
            next_assignees,
        )
        .await?;
    }

    if let Some(state) = edit.update.state.as_deref() {
        if state != edit.current.state {
            let action = if state == "closed" { "close" } else { "reopen" };
            target
                .session
                .update_issue_state(target.owner, target.repo, target.number, action)
                .await?;
        }
    }

    match fetch_after().await {
        Ok((number, title)) => {
            println!("✓ Updated {} #{}: {}", edit.success_noun, number, title);
        }
        Err(err) => {
            eprintln!(
                "Warning: failed to fetch updated {} #{} from API after web fallback: {}",
                edit.success_noun, target.number, err
            );
            println!(
                "✓ Updated {} #{}: {}",
                edit.success_noun, target.number, next_title
            );
        }
    }

    Ok(())
}

pub(crate) async fn latest_comment_by_current_user(
    client: &ApiClient,
    owner: &str,
    repo: &str,
    number: u64,
    item_label: &str,
) -> Result<Comment> {
    let user = client.current_user().await?;
    let comments = client.list_all_issue_comments(owner, repo, number).await?;
    comments
        .into_iter()
        .filter(|comment| {
            comment
                .user
                .as_ref()
                .is_some_and(|comment_user| comment_user.login == user.login)
        })
        .max_by_key(|comment| comment.id)
        .ok_or_else(|| {
            GbError::Other(format!(
                "No comments by {} found on {} #{}",
                user.login, item_label, number
            ))
        })
}
