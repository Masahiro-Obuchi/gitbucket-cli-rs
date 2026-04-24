use dialoguer::{Confirm, Input};

use crate::cli::common::{
    create_client, create_web_session, normalize_edit_state, resolve_hostname, resolve_repo,
};
use crate::error::{GbError, Result};
use crate::models::milestone::{CreateMilestone, UpdateMilestone};

use super::due_date::{
    due_on_to_form_date, normalize_due_on_for_create, normalize_due_on_for_edit, DueOnInput,
};

pub(super) async fn create(
    hostname: &Option<String>,
    cli_repo: &Option<String>,
    cli_profile: &Option<String>,
    title: Option<String>,
    description: Option<String>,
    due_on: Option<String>,
) -> Result<()> {
    let hostname = resolve_hostname(hostname, cli_profile)?;
    let (owner, repo) = resolve_repo(cli_repo, cli_profile)?;
    let client = create_client(&hostname, cli_profile)?;

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

    match client.create_milestone(&owner, &repo, &create_body).await {
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
            let session = create_web_session(&hostname, cli_profile).await?;
            session
                .create_milestone(
                    &owner,
                    &repo,
                    &title,
                    description.as_deref(),
                    normalized_due_on
                        .as_ref()
                        .map(|value| value.form_value.as_str()),
                )
                .await?;
            if let Ok(milestones) = client.list_milestones(&owner, &repo, "all").await {
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
            Ok(())
        }
        Err(err) => Err(err),
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) async fn edit(
    hostname: &Option<String>,
    cli_repo: &Option<String>,
    cli_profile: &Option<String>,
    number: u64,
    title: Option<String>,
    description: Option<String>,
    due_on: Option<String>,
    state: Option<String>,
) -> Result<()> {
    let hostname = resolve_hostname(hostname, cli_profile)?;
    let (owner, repo) = resolve_repo(cli_repo, cli_profile)?;
    let due_on = normalize_due_on_for_edit(due_on)?;
    let state = normalize_edit_state("milestone", state)?;
    if title.is_none()
        && description.is_none()
        && matches!(due_on, DueOnInput::Unchanged)
        && state.is_none()
    {
        return Err(GbError::Other(
            "No milestone changes requested. Pass at least one of --title, --description, --due-on, or --state."
                .into(),
        ));
    }

    let client = create_client(&hostname, cli_profile)?;
    let current = client.get_milestone(&owner, &repo, number).await?;

    let update_body = UpdateMilestone {
        title: title.clone(),
        description: description.clone(),
        due_on: match &due_on {
            DueOnInput::Unchanged => None,
            DueOnInput::Clear => Some(String::new()),
            DueOnInput::Set(value) => Some(value.api_value.clone()),
        },
        state: state.clone(),
    };

    match client
        .update_milestone(&owner, &repo, number, &update_body)
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
            let session = create_web_session(&hostname, cli_profile).await?;
            if title.is_some() || description.is_some() || !matches!(due_on, DueOnInput::Unchanged)
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
                        &owner,
                        &repo,
                        number,
                        title.as_deref().unwrap_or(&current.title),
                        description.as_deref().or(current.description.as_deref()),
                        fallback_due_date.as_deref(),
                    )
                    .await?;
            }

            if let Some(state) = state.as_deref() {
                if state != current.state.to_lowercase() {
                    let action = if state == "closed" { "close" } else { "open" };
                    session
                        .update_milestone_state(&owner, &repo, number, action)
                        .await?;
                }
            }

            let milestone = client.get_milestone(&owner, &repo, number).await?;
            println!(
                "✓ Updated milestone #{}: {}",
                milestone.number, milestone.title
            );
            Ok(())
        }
        Err(err) => Err(err),
    }
}

pub(super) async fn delete(
    hostname: &Option<String>,
    cli_repo: &Option<String>,
    cli_profile: &Option<String>,
    number: u64,
    yes: bool,
) -> Result<()> {
    let hostname = resolve_hostname(hostname, cli_profile)?;
    let (owner, repo) = resolve_repo(cli_repo, cli_profile)?;

    if !yes {
        let confirmed = Confirm::new()
            .with_prompt(format!(
                "Are you sure you want to delete milestone #{} from {}/{}?",
                number, owner, repo
            ))
            .default(false)
            .interact()?;
        if !confirmed {
            println!("Aborted.");
            return Ok(());
        }
    }

    let client = create_client(&hostname, cli_profile)?;
    match client.delete_milestone(&owner, &repo, number).await {
        Ok(()) => {
            println!("✓ Deleted milestone #{}", number);
            Ok(())
        }
        Err(GbError::Api { status: 404, .. }) => {
            eprintln!(
                "Notice: REST milestone delete is unavailable on this GitBucket instance; using web fallback."
            );
            let session = create_web_session(&hostname, cli_profile).await?;
            session.delete_milestone(&owner, &repo, number).await?;
            println!("✓ Deleted milestone #{}", number);
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
