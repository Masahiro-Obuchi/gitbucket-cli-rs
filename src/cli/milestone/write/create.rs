use dialoguer::Input;

use crate::cli::common::{create_web_session, RepoContext};
use crate::error::{GbError, Result};
use crate::models::milestone::{CreateMilestone, UpdateMilestone};

use super::super::due_date::normalize_due_on_for_create;
use super::values::{rest_title, validate_title, validate_web_text, verify_web_result};

pub(in crate::cli::milestone) async fn create(
    hostname: &Option<String>,
    cli_repo: &Option<String>,
    cli_profile: &Option<String>,
    title: Option<String>,
    description: Option<String>,
    due_on: Option<String>,
) -> Result<()> {
    let ctx = RepoContext::resolve(hostname, cli_repo, cli_profile)?;
    let title = match title {
        Some(title) => title,
        None => Input::new()
            .with_prompt("Milestone title")
            .interact_text()?,
    };
    validate_title(&title)?;
    let due_on = normalize_due_on_for_create(due_on)?;
    let body = CreateMilestone {
        title: title.clone(),
        description: description.clone(),
        due_on: due_on.as_ref().map(|value| value.api_value.clone()),
    };
    let milestone = match ctx
        .client
        .create_milestone(&ctx.owner, &ctx.repo, &body)
        .await
    {
        Ok(milestone) => milestone,
        Err(GbError::Api { status: 404, .. }) if !rest_title(&title) => {
            validate_web_text(&title, description.as_deref())?;
            // A readable repository and absent title are required before asking
            // for Web credentials. Never diagnose an arbitrary 404 as no REST API.
            let before = ctx
                .client
                .list_milestones(&ctx.owner, &ctx.repo, "all")
                .await?;
            if before
                .iter()
                .any(|milestone| milestone.title.to_lowercase() == title.to_lowercase())
            {
                return Err(GbError::Other(
                    "A milestone with this title already exists; no Web create was attempted."
                        .into(),
                ));
            }
            eprintln!("Notice: GitBucket 4.44 REST rejects this milestone title; using the Web route for the same title.");
            let session = create_web_session(&ctx.hostname, cli_profile).await?;
            session
                .create_milestone(
                    &ctx.owner,
                    &ctx.repo,
                    &title,
                    description.as_deref(),
                    due_on.as_ref().map(|value| value.form_value.as_str()),
                )
                .await?;
            let after = ctx.client.list_milestones(&ctx.owner, &ctx.repo, "all").await.map_err(|err|
                GbError::Other(format!("Milestone Web create was submitted, but the result could not be verified: {err}. Inspect the milestone list before retrying.")))?;
            let milestone = after.into_iter().find(|milestone| milestone.title == title
                && !before.iter().any(|old| old.number == milestone.number)).ok_or_else(||
                GbError::Other("Milestone Web create was submitted, but no new milestone with the requested title was found. Inspect the milestone list before retrying.".into()))?;
            verify_web_result(
                &milestone,
                &UpdateMilestone {
                    title,
                    description,
                    due_on: body.due_on,
                    state: "open".into(),
                },
            )?;
            milestone
        }
        Err(err) => return Err(err),
    };
    println!(
        "✓ Created milestone #{}: {}",
        milestone.number, milestone.title
    );
    Ok(())
}
