use crate::cli::common::{create_web_session, normalize_edit_state, RepoContext};
use crate::error::{GbError, Result};
use crate::models::milestone::UpdateMilestone;

use super::super::due_date::{due_on_to_form_date, normalize_due_on_for_edit, DueOnInput};

pub(in crate::cli::milestone) struct EditRequest {
    pub number: u64,
    pub title: Option<String>,
    pub description: Option<String>,
    pub due_on: Option<String>,
    pub state: Option<String>,
}

pub(in crate::cli::milestone) async fn edit(
    hostname: &Option<String>,
    cli_repo: &Option<String>,
    cli_profile: &Option<String>,
    request: EditRequest,
) -> Result<()> {
    let ctx = RepoContext::resolve(hostname, cli_repo, cli_profile)?;
    let due_on = normalize_due_on_for_edit(request.due_on)?;
    let state = normalize_edit_state("milestone", request.state)?;
    if request.title.is_none()
        && request.description.is_none()
        && matches!(due_on, DueOnInput::Unchanged)
        && state.is_none()
    {
        return Err(GbError::Other(
            "No milestone changes requested. Pass at least one of --title, --description, --due-on, or --state."
                .into(),
        ));
    }

    let current = ctx
        .client
        .get_milestone(&ctx.owner, &ctx.repo, request.number)
        .await?;

    let update_body = UpdateMilestone {
        title: request.title.clone(),
        description: request.description.clone(),
        due_on: match &due_on {
            DueOnInput::Unchanged => None,
            DueOnInput::Clear => Some(String::new()),
            DueOnInput::Set(value) => Some(value.api_value.clone()),
        },
        state: state.clone(),
    };

    match ctx
        .client
        .update_milestone(&ctx.owner, &ctx.repo, request.number, &update_body)
        .await
    {
        Ok(milestone) => {
            println!(
                "✓ Updated milestone #{}: {}",
                milestone.number, milestone.title
            );
            Ok(())
        }
        Err(GbError::Api { status: 404, .. }) => {
            eprintln!(
                "Notice: REST milestone edit is unavailable on this GitBucket instance; using web fallback."
            );
            let session = create_web_session(&ctx.hostname, cli_profile).await?;
            if request.title.is_some()
                || request.description.is_some()
                || !matches!(due_on, DueOnInput::Unchanged)
            {
                let fallback_due_date = match &due_on {
                    DueOnInput::Unchanged => current
                        .due_on
                        .as_deref()
                        .map(due_on_to_form_date)
                        .transpose()?,
                    DueOnInput::Clear => None,
                    DueOnInput::Set(value) => Some(value.form_value.clone()),
                };
                session
                    .edit_milestone(
                        &ctx.owner,
                        &ctx.repo,
                        request.number,
                        request.title.as_deref().unwrap_or(&current.title),
                        request
                            .description
                            .as_deref()
                            .or(current.description.as_deref()),
                        fallback_due_date.as_deref(),
                    )
                    .await?;
            }

            if let Some(state) = state.as_deref() {
                if state != current.state.to_lowercase() {
                    let action = if state == "closed" { "close" } else { "open" };
                    session
                        .update_milestone_state(&ctx.owner, &ctx.repo, request.number, action)
                        .await?;
                }
            }

            let milestone = ctx
                .client
                .get_milestone(&ctx.owner, &ctx.repo, request.number)
                .await?;
            println!(
                "✓ Updated milestone #{}: {}",
                milestone.number, milestone.title
            );
            Ok(())
        }
        Err(err) => Err(err),
    }
}

#[cfg(test)]
mod tests {
    use crate::cli::common::normalize_edit_state;

    #[test]
    fn normalize_edit_state_rejects_all() {
        assert!(normalize_edit_state("milestone", Some("all".into())).is_err());
    }
}
