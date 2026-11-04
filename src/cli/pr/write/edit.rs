use std::collections::BTreeSet;

use crate::cli::common::{
    create_web_session, merge_named_values, normalize_edit_state, update_issue_assignees_via_web,
    RepoContext,
};
use crate::error::{GbError, Result};
use crate::models::issue::Issue;
use crate::models::pull_request::{PullRequest, UpdatePullRequest};

pub(in crate::cli::pr) struct EditRequest {
    pub number: u64,
    pub title: Option<String>,
    pub body: Option<String>,
    pub add_assignees: Vec<String>,
    pub remove_assignees: Vec<String>,
    pub state: Option<String>,
    pub web: bool,
}

impl EditRequest {
    pub(super) fn state(number: u64, state: &str) -> Self {
        Self {
            number,
            title: None,
            body: None,
            add_assignees: vec![],
            remove_assignees: vec![],
            state: Some(state.into()),
            web: false,
        }
    }
}

pub(in crate::cli::pr) async fn edit(
    hostname: &Option<String>,
    cli_repo: &Option<String>,
    cli_profile: &Option<String>,
    request: EditRequest,
) -> Result<()> {
    let pr = apply_edit(hostname, cli_repo, cli_profile, request).await?;
    println!("✓ Updated pull request #{}: {}", pr.number, pr.title);
    Ok(())
}

fn assignee_names(issue: &Issue) -> BTreeSet<String> {
    issue
        .assignees
        .iter()
        .map(|user| user.login.clone())
        .collect()
}

fn check_issue_number(issue: &Issue, number: u64) -> Result<()> {
    if issue.number != number {
        return Err(GbError::Other(
            "PR assignee response belongs to a different number; refusing to continue.".into(),
        ));
    }
    Ok(())
}

pub(super) async fn apply_edit(
    hostname: &Option<String>,
    cli_repo: &Option<String>,
    cli_profile: &Option<String>,
    request: EditRequest,
) -> Result<PullRequest> {
    if request.title.is_none()
        && request.body.is_none()
        && request.add_assignees.is_empty()
        && request.remove_assignees.is_empty()
        && request.state.is_none()
    {
        return Err(GbError::Other(
            "No pull request changes requested. Pass at least one edit option.".into(),
        ));
    }
    if request
        .title
        .as_deref()
        .is_some_and(|title| title.trim().is_empty())
    {
        return Err(GbError::Other(
            "Pull request title must not be blank.".into(),
        ));
    }
    let state = normalize_edit_state("pull request", request.state)?;
    let ctx = RepoContext::resolve(hostname, cli_repo, cli_profile)?;
    let current = super::super::target::require_pull_request(&ctx, request.number).await?;
    if state.as_deref() == Some("open")
        && (current.merged == Some(true) || current.merged_at.is_some())
    {
        return Err(GbError::Other(
            "Cannot reopen a merged pull request. No changes were applied.".into(),
        ));
    }

    // GitBucket 4.44 updates content only inside title.foreach. Body-only input
    // needs the current title, while title-only input must retain the stored body.
    // State-only requests intentionally omit both content fields.
    let edits_content = request.title.is_some() || request.body.is_some();
    let update = UpdatePullRequest {
        title: if edits_content {
            Some(request.title.unwrap_or_else(|| current.title.clone()))
        } else {
            None
        },
        body: if edits_content {
            request.body.or_else(|| current.body.clone())
        } else {
            None
        },
        state: state.filter(|state| *state != current.state),
    };

    let assignees = if request.add_assignees.is_empty() && request.remove_assignees.is_empty() {
        None
    } else {
        let issue = ctx
            .client
            .get_issue(&ctx.owner, &ctx.repo, request.number)
            .await?;
        check_issue_number(&issue, request.number)?;
        let next = merge_named_values(
            issue.assignees.iter().map(|user| user.login.clone()),
            request.add_assignees,
            request.remove_assignees,
        );
        if assignee_names(&issue) == next.iter().cloned().collect() {
            None
        } else {
            Some((issue, next))
        }
    };
    let mut progress = crate::operation::Progress::default();
    let rest_step = (!update.is_empty())
        .then(|| progress.plan("pr.title_body_state", crate::operation::Route::Rest));
    let assignee_step = assignees
        .as_ref()
        .map(|_| progress.plan("pr.assignees", crate::operation::Route::Web));
    // Preflight permission/auth before a mixed edit changes any REST fields.
    let session = if assignees.is_some() {
        if !request.web {
            return Err(GbError::Other("PR assignee changes require --web to permit GitBucket Web UI authentication. No changes were applied; title/body/state alone use REST without --web.".into()));
        }
        if let Some((issue, next)) = &assignees {
            crate::cli::common::preflight_assignees(&ctx.client, issue, next)
                .await
                .map_err(|err| progress.error(err))?;
        }
        Some(
            create_web_session(&ctx.client, &ctx.hostname, cli_profile)
                .await
                .map_err(|err| progress.error(err))?,
        )
    } else {
        None
    };

    if let Some(session) = &session {
        progress.actor(session.principal());
    }
    let updated = if let Some(step) = rest_step {
        progress
            .run(
                step,
                ctx.client
                    .update_pull_request(&ctx.owner, &ctx.repo, request.number, &update),
                |updated| async {
                    verify_rest_update(&updated, &current, &update)?;
                    Ok(updated)
                },
            )
            .await?
    } else {
        current.clone()
    };

    if let (Some((issue, next)), Some(session), Some(step)) = (assignees, session, assignee_step) {
        progress
            .run(
                step,
                update_issue_assignees_via_web(
                    &ctx.client,
                    &session,
                    &ctx.owner,
                    &ctx.repo,
                    request.number,
                    &issue,
                    &next,
                ),
                |_| async { Ok(()) },
            )
            .await?;
    }
    Ok(updated)
}

fn verify_rest_update(
    actual: &PullRequest,
    before: &PullRequest,
    update: &UpdatePullRequest,
) -> Result<()> {
    let title = update.title.as_deref().unwrap_or(&before.title);
    let body = if update.title.is_some() {
        update.body.as_deref()
    } else {
        before.body.as_deref()
    };
    let state = update.state.as_deref().unwrap_or(&before.state);
    if actual.number != before.number
        || actual.title != title
        || actual.body.as_deref().unwrap_or_default() != body.unwrap_or_default()
        || actual.state != state
    {
        return Err(GbError::Other("PR REST update returned fields that do not match the intended result. Inspect the PR before retrying; changes may already be applied.".into()));
    }
    Ok(())
}
