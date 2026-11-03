use crate::api::client::ApiClient;
use crate::api::web::GitBucketWebSession;
use crate::cli::common::update_issue_assignees_via_web;
use crate::error::{GbError, Result};
use crate::models::comment::Comment;
use crate::models::issue::{Issue, UpdateIssue};

pub(crate) struct IssueLikeEdit<'a> {
    pub current: &'a IssueLikeCurrent,
    pub update: &'a UpdateIssue,
    pub rest_unavailable_notice: &'a str,
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
}

pub(crate) async fn apply_issue_like_edit(
    target: IssueLikeTarget<'_>,
    edit: IssueLikeEdit<'_>,
) -> Result<()> {
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
