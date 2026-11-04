use crate::cli::common::{
    create_web_session, merge_named_values, normalize_edit_state, RepoContext,
};
use crate::cli::issue_like::{
    apply_issue_like_edit, IssueLikeCurrent, IssueLikeEdit, IssueLikeTarget,
};
use crate::error::{GbError, Result};
use crate::models::issue::UpdateIssue;

pub(in crate::cli::issue) struct EditRequest {
    pub number: u64,
    pub title: Option<String>,
    pub body: Option<String>,
    pub add_labels: Vec<String>,
    pub remove_labels: Vec<String>,
    pub add_assignees: Vec<String>,
    pub remove_assignees: Vec<String>,
    pub milestone: Option<u64>,
    pub remove_milestone: bool,
    pub state: Option<String>,
}

pub(in crate::cli::issue) async fn edit(
    hostname: &Option<String>,
    cli_repo: &Option<String>,
    cli_profile: &Option<String>,
    request: EditRequest,
) -> Result<()> {
    if request.title.is_none()
        && request.body.is_none()
        && request.add_labels.is_empty()
        && request.remove_labels.is_empty()
        && request.add_assignees.is_empty()
        && request.remove_assignees.is_empty()
        && request.milestone.is_none()
        && !request.remove_milestone
        && request.state.is_none()
    {
        return Err(GbError::Other(
            "No issue changes requested. Pass at least one edit option.".into(),
        ));
    }

    if request.milestone.is_some() && request.remove_milestone {
        return Err(GbError::Other(
            "Cannot use --milestone and --remove-milestone together.".into(),
        ));
    }

    let state = normalize_edit_state("issue", request.state)?;
    let ctx = RepoContext::resolve(hostname, cli_repo, cli_profile)?;
    let current = ctx
        .client
        .get_issue(&ctx.owner, &ctx.repo, request.number)
        .await?;

    if current.number != request.number {
        return Err(GbError::Other(
            "Issue lookup returned another number. No changes were applied.".into(),
        ));
    }
    let labels = super::labels::LabelChanges::new(request.add_labels, request.remove_labels)?;
    labels.preflight(&ctx).await?;

    let assignees = if request.add_assignees.is_empty() && request.remove_assignees.is_empty() {
        None
    } else {
        Some(merge_named_values(
            current
                .assignees
                .iter()
                .map(|assignee| assignee.login.clone()),
            request.add_assignees,
            request.remove_assignees,
        ))
    };

    let milestone = if request.remove_milestone {
        Some(None)
    } else {
        request.milestone.map(Some)
    };

    let update_body = UpdateIssue {
        state,
        title: request.title,
        body: request.body,
        assignees,
        milestone,
    };

    let metadata_requested = update_body.title.is_some()
        || update_body.body.is_some()
        || update_body.state.is_some()
        || update_body.assignees.is_some()
        || update_body.milestone.is_some();
    let mut stored = current.clone();
    if metadata_requested {
        match ctx
            .client
            .update_issue(&ctx.owner, &ctx.repo, request.number, &update_body)
            .await
        {
            Ok(issue) => {
                if issue.number != request.number {
                    return Err(GbError::Other("Issue metadata request was submitted, but response number mismatches. Inspect the Issue before retrying.".into()));
                }
            }
            Err(GbError::Api { status: 404, .. }) => {
                let session = create_web_session(&ctx.hostname, cli_profile).await?;
                let current = IssueLikeCurrent::issue(current.clone());
                apply_issue_like_edit(
                    IssueLikeTarget { session: &session, owner: &ctx.owner, repo: &ctx.repo, number: request.number },
                    IssueLikeEdit {
                        current: &current, update: &update_body,
                        rest_unavailable_notice: "Notice: REST issue metadata edit is unavailable; using Web for metadata only. Labels use their dedicated REST endpoints.",
                    },
                ).await.map_err(|err| GbError::Other(format!("Issue Web metadata update failed: {err}. Metadata may be partially applied; no label changes were attempted. Inspect the Issue before retrying.")))?;
            }
            Err(err) => return Err(err),
        }
    }
    if metadata_requested {
        stored = read_after_edit(&ctx, request.number).await?;
        verify_metadata(&stored, request.number, &update_body)?;
    }
    if !labels.is_empty() {
        labels.apply(&ctx, request.number).await.map_err(|err| {
            if metadata_requested {
                GbError::Other(format!(
                    "Metadata update steps completed before label processing failed: {err}"
                ))
            } else {
                err
            }
        })?;
    }
    if !labels.is_empty() {
        stored = read_after_edit(&ctx, request.number).await?;
        labels.verify(&stored.labels)?;
    }
    verify_metadata(&stored, request.number, &update_body)?;
    println!("✓ Updated issue #{}: {}", stored.number, stored.title);
    Ok(())
}

async fn read_after_edit(ctx: &RepoContext, number: u64) -> Result<crate::models::issue::Issue> {
    ctx.client.get_issue(&ctx.owner, &ctx.repo, number).await.map_err(|err|
        GbError::Other(format!("Issue edit steps completed, but verification failed: {err}. Inspect the Issue before retrying.")))
}

fn verify_metadata(
    stored: &crate::models::issue::Issue,
    number: u64,
    update_body: &UpdateIssue,
) -> Result<()> {
    if stored.number != number {
        return Err(GbError::Other("Issue edit was submitted, but verification returned another number. Inspect the Issue before retrying.".into()));
    }
    if update_body
        .title
        .as_deref()
        .is_some_and(|title| title != stored.title)
        || update_body
            .body
            .as_deref()
            .is_some_and(|body| body != stored.body.as_deref().unwrap_or_default())
        || update_body
            .state
            .as_deref()
            .is_some_and(|state| state != stored.state)
        || update_body.milestone.is_some_and(|milestone| {
            milestone != stored.milestone.as_ref().map(|value| value.number)
        })
        || update_body.assignees.as_ref().is_some_and(|next| {
            next.iter().collect::<std::collections::BTreeSet<_>>()
                != stored.assignees.iter().map(|user| &user.login).collect()
        })
    {
        return Err(GbError::Other("Issue edit was submitted, but stored metadata does not match. Inspect the Issue before retrying; changes may be partially applied.".into()));
    }
    Ok(())
}
