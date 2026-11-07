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
    pub client: &'a ApiClient,
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
    use crate::operation::{Progress, Route};
    eprintln!("{}", edit.rest_unavailable_notice);
    let next_title = edit.update.title.as_deref().unwrap_or(&edit.current.title);
    let current_body = edit.current.body.as_deref().unwrap_or_default();
    let next_body = edit.update.body.as_deref().unwrap_or(current_body);
    // Plan every phase before the first write, including phases left untouched
    // when an earlier request or its verification fails.
    let mut progress = Progress::default();
    let title =
        (next_title != edit.current.title).then(|| progress.plan("issue.title", Route::Web));
    let body = (next_body != current_body).then(|| progress.plan("issue.body", Route::Web));
    let milestone = edit
        .update
        .milestone
        .map(|_| progress.plan("issue.milestone", Route::Web));
    let assignees = edit
        .update
        .assignees
        .as_ref()
        .map(|_| progress.plan("issue.assignees", Route::Web));
    let state = edit
        .update
        .state
        .as_ref()
        .filter(|s| **s != edit.current.state)
        .map(|_| progress.plan("issue.state", Route::Web));
    progress.actor(target.session.principal());
    if let Some(step) = title {
        progress
            .run(
                step,
                target.session.edit_issue_title(
                    target.owner,
                    target.repo,
                    target.number,
                    next_title,
                ),
                |_| target.verify(|issue| issue.title == next_title),
            )
            .await?;
    }
    if let Some(step) = body {
        progress
            .run(
                step,
                target.session.edit_issue_content(
                    target.owner,
                    target.repo,
                    target.number,
                    next_title,
                    next_body,
                ),
                |_| {
                    target.verify(|issue| {
                        issue.title == next_title
                            && issue.body.as_deref().unwrap_or_default() == next_body
                    })
                },
            )
            .await?;
    }
    if let (Some(step), Some(value)) = (milestone, edit.update.milestone) {
        progress
            .run(
                step,
                target.session.update_issue_milestone(
                    target.owner,
                    target.repo,
                    target.number,
                    value,
                ),
                |_| target.verify(|issue| issue.milestone.as_ref().map(|m| m.number) == value),
            )
            .await?;
    }
    if let (Some(step), Some(next)) = (assignees, &edit.update.assignees) {
        let current = edit.current.issue.as_ref().ok_or_else(|| {
            progress.error(GbError::Other(
                "Issue details required for assignee updates.".into(),
            ))
        })?;
        progress
            .run(
                step,
                update_issue_assignees_via_web(
                    target.client,
                    target.session,
                    target.owner,
                    target.repo,
                    target.number,
                    current,
                    next,
                ),
                |_| async { Ok(()) },
            )
            .await?;
    }
    if let (Some(step), Some(value)) = (state, &edit.update.state) {
        let action = if value == "closed" { "close" } else { "reopen" };
        progress
            .run(
                step,
                target
                    .session
                    .update_issue_state(target.owner, target.repo, target.number, action),
                |_| target.verify(|issue| issue.state == *value),
            )
            .await?;
    }
    Ok(())
}

impl IssueLikeTarget<'_> {
    async fn verify(&self, matches: impl FnOnce(&Issue) -> bool) -> Result<()> {
        let stored = self
            .client
            .get_issue(self.owner, self.repo, self.number)
            .await?;
        if stored.number != self.number || !matches(&stored) {
            return Err(GbError::Other(
                "Issue Web update was submitted, but stored fields do not match.".into(),
            ));
        }
        Ok(())
    }
}

pub(crate) async fn latest_comment_by_current_user(
    client: &ApiClient,
    owner: &str,
    repo: &str,
    number: u64,
    item_label: &str,
) -> Result<Comment> {
    let user = client.current_user().await?;
    let comments = client.list_issue_comments(owner, repo, number).await?;
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
