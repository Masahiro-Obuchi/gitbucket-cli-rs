use crate::cli::common::{create_web_session, normalize_edit_state, RepoContext};
use crate::error::{GbError, Result};
use crate::models::milestone::{Milestone, UpdateMilestone};

use super::super::change::Change;
use super::super::due_date::{due_on_to_form_date, normalize_due_on_for_edit, DueOnInput};
use super::values::{
    read_after_web, rest_title, validate_title, validate_web_text, verify_web_result,
};

pub(in crate::cli::milestone) struct EditRequest {
    pub number: u64,
    pub title: Option<String>,
    pub description: Option<String>,
    pub due_on: Option<String>,
    pub state: Option<String>,
}

struct Changes {
    title: Option<String>,
    description: Change<String>,
    due_on: DueOnInput,
    state: Option<String>,
}

impl Changes {
    fn merge(&self, current: &Milestone) -> UpdateMilestone {
        UpdateMilestone {
            title: self.title.clone().unwrap_or_else(|| current.title.clone()),
            description: self.description.merge(&current.description),
            due_on: match &self.due_on {
                Change::Unchanged => current.due_on.clone(),
                Change::Clear => None,
                Change::Set(value) => Some(value.api_value.clone()),
            },
            state: self.state.clone().unwrap_or_else(|| current.state.clone()),
        }
    }

    fn edits_content(&self) -> bool {
        self.title.is_some()
            || !matches!(self.description, Change::Unchanged)
            || !matches!(self.due_on, Change::Unchanged)
    }
}

pub(in crate::cli::milestone) async fn edit(
    hostname: &Option<String>,
    cli_repo: &Option<String>,
    cli_profile: &Option<String>,
    request: EditRequest,
) -> Result<()> {
    let changes = Changes {
        title: request.title,
        description: Change::text(request.description),
        due_on: normalize_due_on_for_edit(request.due_on)?,
        state: normalize_edit_state("milestone", request.state)?,
    };
    if !changes.edits_content() && changes.state.is_none() {
        return Err(GbError::Other("No milestone changes requested. Pass at least one of --title, --description, --due-on, or --state.".into()));
    }
    if let Some(title) = &changes.title {
        validate_title(title)?;
    }
    let ctx = RepoContext::resolve(hostname, cli_repo, cli_profile)?;
    let current = ctx
        .client
        .get_milestone(&ctx.owner, &ctx.repo, request.number)
        .await?;
    // Repeating a state-only request needs neither replacement nor Web auth.
    if !changes.edits_content() && changes.state.as_deref() == Some(current.state.as_str()) {
        println!("✓ Updated milestone #{}: {}", current.number, current.title);
        return Ok(());
    }
    let body = changes.merge(&current);
    validate_title(&body.title)?;
    let milestone = match ctx
        .client
        .update_milestone(&ctx.owner, &ctx.repo, request.number, &body)
        .await
    {
        Ok(milestone) => milestone,
        // 4.44's known title validation failure, never a generic "404 = missing API".
        Err(GbError::Api { status: 404, .. }) if !rest_title(&body.title) => {
            // Confirm it still exists and merge the latest values before Web I/O.
            let current = ctx
                .client
                .get_milestone(&ctx.owner, &ctx.repo, request.number)
                .await?;
            let expected = changes.merge(&current);
            if rest_title(&expected.title) {
                // Another editor may have changed the title since the rejected
                // request. A fresh compatible title no longer needs Web auth.
                ctx.client
                    .update_milestone(&ctx.owner, &ctx.repo, request.number, &expected)
                    .await?
            } else {
                edit_via_web(
                    &ctx,
                    cli_profile,
                    request.number,
                    &changes,
                    &current,
                    &expected,
                )
                .await?
            }
        }
        Err(err) => return Err(err),
    };
    println!(
        "✓ Updated milestone #{}: {}",
        milestone.number, milestone.title
    );
    Ok(())
}

async fn edit_via_web(
    ctx: &RepoContext,
    cli_profile: &Option<String>,
    number: u64,
    changes: &Changes,
    current: &Milestone,
    expected: &UpdateMilestone,
) -> Result<Milestone> {
    if !changes.edits_content() && expected.state == current.state {
        return Ok(current.clone());
    }
    let due_date = if changes.edits_content() {
        validate_web_text(&expected.title, expected.description.as_deref())?;
        if matches!(changes.due_on, Change::Unchanged)
            && current
                .due_on
                .as_deref()
                .is_some_and(|value| !value.starts_with("0001-01-01"))
        {
            return Err(GbError::Other("The milestone Web form accepts only a calendar date and cannot guarantee preservation of an existing due timestamp. Pass --due-on explicitly (or an empty string to clear), or use a REST-compatible title.".into()));
        }
        expected
            .due_on
            .as_deref()
            .map(due_on_to_form_date)
            .transpose()?
    } else {
        None
    };
    eprintln!("Notice: GitBucket 4.44 REST rejects this milestone title; using the Web route for the same title.");
    let session = create_web_session(&ctx.hostname, cli_profile).await?;
    if changes.edits_content() {
        session
            .edit_milestone(
                &ctx.owner,
                &ctx.repo,
                number,
                &expected.title,
                expected.description.as_deref(),
                due_date.as_deref(),
            )
            .await?;
    }
    if expected.state != current.state {
        let action = if expected.state == "closed" {
            "close"
        } else {
            "open"
        };
        session
            .update_milestone_state(&ctx.owner, &ctx.repo, number, action)
            .await
            .map_err(|err| {
                let progress = if changes.edits_content() {
                    "Content changes may already be applied. "
                } else {
                    ""
                };
                GbError::Other(format!(
                    "Milestone Web state update failed: {err}. {progress}Inspect the milestone before retrying."
                ))
            })?;
    }
    let actual = read_after_web(ctx, number).await?;
    verify_web_result(&actual, expected)?;
    Ok(actual)
}
