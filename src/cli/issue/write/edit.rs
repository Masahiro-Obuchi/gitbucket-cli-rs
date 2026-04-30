use crate::cli::common::{
    create_web_session, merge_named_values, normalize_edit_state, RepoContext,
};
use crate::cli::issue_like::{
    run_issue_like_edit, IssueLikeCurrent, IssueLikeEdit, IssueLikeTarget,
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

    let labels = if request.add_labels.is_empty() && request.remove_labels.is_empty() {
        None
    } else {
        Some(merge_named_values(
            current.labels.iter().map(|label| label.name.clone()),
            request.add_labels,
            request.remove_labels,
        ))
    };

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
        labels,
        assignees,
        milestone,
    };

    match ctx
        .client
        .update_issue(&ctx.owner, &ctx.repo, request.number, &update_body)
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

            let session = create_web_session(&ctx.hostname, cli_profile).await?;
            let current = IssueLikeCurrent::issue(current);
            run_issue_like_edit(
                IssueLikeTarget {
                    session: &session,
                    owner: &ctx.owner,
                    repo: &ctx.repo,
                    number: request.number,
                },
                IssueLikeEdit {
                    current: &current,
                    update: &update_body,
                    allow_web_fallback: true,
                    unsupported_web_fallback_message: Some(
                        "This GitBucket instance does not support editing issue labels through the web fallback; the web fallback cannot edit labels. Retry against an instance with REST issue edit support, or update title/body/assignees/milestone/state only.",
                    ),
                    rest_unavailable_notice:
                        "Notice: REST issue edit is unavailable on this GitBucket instance; using web fallback.",
                    success_noun: "issue",
                },
                || async {
                    let issue = ctx
                        .client
                        .get_issue(&ctx.owner, &ctx.repo, request.number)
                        .await?;
                    Ok((issue.number, issue.title))
                },
            )
            .await
        }
        Err(err) => Err(err),
    }
}
