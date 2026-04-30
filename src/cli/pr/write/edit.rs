use crate::cli::common::{
    create_web_session, merge_named_values, normalize_edit_state, RepoContext,
};
use crate::cli::issue_like::{
    run_issue_like_edit, IssueLikeCurrent, IssueLikeEdit, IssueLikeTarget,
};
use crate::error::{GbError, Result};
use crate::models::issue::UpdateIssue;

pub(in crate::cli::pr) struct EditRequest {
    pub number: u64,
    pub title: Option<String>,
    pub body: Option<String>,
    pub add_assignees: Vec<String>,
    pub remove_assignees: Vec<String>,
    pub state: Option<String>,
    pub web: bool,
}

pub(in crate::cli::pr) async fn edit(
    hostname: &Option<String>,
    cli_repo: &Option<String>,
    cli_profile: &Option<String>,
    request: EditRequest,
) -> Result<()> {
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

    let state = normalize_edit_state("pull request", request.state)?;
    let ctx = RepoContext::resolve(hostname, cli_repo, cli_profile)?;

    let current_pr = ctx
        .client
        .get_pull_request(&ctx.owner, &ctx.repo, request.number)
        .await?;

    let current_issue = if request.add_assignees.is_empty() && request.remove_assignees.is_empty() {
        None
    } else {
        Some(
            ctx.client
                .get_issue(&ctx.owner, &ctx.repo, request.number)
                .await?,
        )
    };

    let assignees = current_issue.as_ref().map(|current| {
        merge_named_values(
            current
                .assignees
                .iter()
                .map(|assignee| assignee.login.clone()),
            request.add_assignees,
            request.remove_assignees,
        )
    });

    let update_body = UpdateIssue {
        state,
        title: request.title,
        body: request.body,
        labels: None,
        assignees,
        milestone: None,
    };

    match ctx
        .client
        .update_issue(&ctx.owner, &ctx.repo, request.number, &update_body)
        .await
    {
        Ok(issue) => {
            println!("✓ Updated pull request #{}: {}", issue.number, issue.title);
            Ok(())
        }
        Err(GbError::Api { status: 404, .. }) => {
            if !request.web {
                return Err(GbError::Other(
                    "REST PR edit is unavailable on this GitBucket instance. Re-run with --web to allow the GitBucket web UI fallback.".into(),
                ));
            }

            let session = create_web_session(&ctx.hostname, cli_profile).await?;
            let current = IssueLikeCurrent::pull_request(
                current_pr.title,
                current_pr.body,
                current_pr.state,
                current_issue,
            );
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
                    allow_web_fallback: request.web,
                    unsupported_web_fallback_message: Some(
                        "REST PR edit is unavailable on this GitBucket instance. Re-run with --web to allow the GitBucket web UI fallback.",
                    ),
                    rest_unavailable_notice:
                        "Notice: REST PR edit is unavailable on this GitBucket instance; using web fallback.",
                    success_noun: "pull request",
                },
                || async {
                    let pr = ctx
                        .client
                        .get_pull_request(&ctx.owner, &ctx.repo, request.number)
                        .await?;
                    Ok((pr.number, pr.title))
                },
            )
            .await
        }
        Err(err) => Err(err),
    }
}
