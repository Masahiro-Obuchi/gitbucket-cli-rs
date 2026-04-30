use dialoguer::Input;

use crate::cli::common::{create_web_session, RepoContext};
use crate::error::{GbError, Result};
use crate::models::milestone::CreateMilestone;

use super::super::due_date::normalize_due_on_for_create;

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

    let normalized_due_on = normalize_due_on_for_create(due_on)?;
    let create_body = CreateMilestone {
        title: title.clone(),
        description: description.clone(),
        due_on: normalized_due_on
            .as_ref()
            .map(|value| value.api_value.clone()),
    };

    match ctx
        .client
        .create_milestone(&ctx.owner, &ctx.repo, &create_body)
        .await
    {
        Ok(milestone) => {
            println!(
                "✓ Created milestone #{}: {}",
                milestone.number, milestone.title
            );
            Ok(())
        }
        Err(GbError::Api { status: 404, .. }) => {
            eprintln!(
                "Notice: REST milestone create is unavailable on this GitBucket instance; using web fallback."
            );
            let session = create_web_session(&ctx.hostname, cli_profile).await?;
            session
                .create_milestone(
                    &ctx.owner,
                    &ctx.repo,
                    &title,
                    description.as_deref(),
                    normalized_due_on
                        .as_ref()
                        .map(|value| value.form_value.as_str()),
                )
                .await?;
            print_created_after_web_fallback(&ctx, &title).await;
            Ok(())
        }
        Err(err) => Err(err),
    }
}

async fn print_created_after_web_fallback(ctx: &RepoContext, title: &str) {
    if let Ok(milestones) = ctx
        .client
        .list_milestones(&ctx.owner, &ctx.repo, "all")
        .await
    {
        if let Some(milestone) = milestones
            .iter()
            .rev()
            .find(|milestone| milestone.title == title)
        {
            println!(
                "✓ Created milestone #{}: {}",
                milestone.number, milestone.title
            );
        } else {
            println!("✓ Created milestone {}", title);
        }
    } else {
        println!("✓ Created milestone {}", title);
    }
}
