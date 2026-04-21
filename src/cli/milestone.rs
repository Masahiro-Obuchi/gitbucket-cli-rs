use clap::{Args, Subcommand};
use colored::Colorize;
use dialoguer::{Confirm, Input};
use time::format_description::well_known::Rfc3339;
use time::macros::format_description;
use time::{Date, OffsetDateTime};

use crate::cli::common::{
    create_client, create_web_session, normalize_list_state, resolve_hostname, resolve_repo,
};
use crate::error::{GbError, Result};
use crate::models::milestone::{CreateMilestone, UpdateMilestone};
use crate::output::table::print_table;
use crate::output::{format_state, truncate};

#[derive(Args)]
pub struct MilestoneArgs {
    #[command(subcommand)]
    pub command: MilestoneCommand,
}

#[derive(Subcommand)]
pub enum MilestoneCommand {
    /// List milestones
    List {
        /// Filter by state (open, closed, all)
        #[arg(
            long,
            short,
            default_value = "open",
            value_parser = ["open", "closed", "all"],
            ignore_case = true
        )]
        state: String,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
    /// View a milestone
    View {
        /// Milestone number
        number: u64,
    },
    /// Create a milestone
    Create {
        /// Milestone title (prompts when omitted)
        title: Option<String>,
        /// Optional milestone description
        #[arg(long, short)]
        description: Option<String>,
        /// Due date as YYYY-MM-DD or RFC3339
        #[arg(long = "due-on")]
        due_on: Option<String>,
    },
    /// Edit a milestone
    Edit {
        /// Milestone number
        number: u64,
        /// Updated title
        #[arg(long, short)]
        title: Option<String>,
        /// Updated description
        #[arg(long, short)]
        description: Option<String>,
        /// Updated due date as YYYY-MM-DD, RFC3339, or an empty string to clear
        #[arg(long = "due-on")]
        due_on: Option<String>,
        /// Updated state (open or closed)
        #[arg(long, short, value_parser = ["open", "closed"], ignore_case = true)]
        state: Option<String>,
    },
    /// Delete a milestone
    Delete {
        /// Milestone number
        number: u64,
        /// Skip confirmation
        #[arg(long)]
        yes: bool,
    },
}

struct NormalizedDueOn {
    api_value: String,
    form_value: String,
}

enum DueOnInput {
    Unchanged,
    Clear,
    Set(NormalizedDueOn),
}

pub async fn run(
    args: MilestoneArgs,
    cli_hostname: &Option<String>,
    cli_repo: &Option<String>,
    cli_profile: &Option<String>,
) -> Result<()> {
    match args.command {
        MilestoneCommand::List { state, json } => {
            list(cli_hostname, cli_repo, cli_profile, &state, json).await
        }
        MilestoneCommand::View { number } => {
            view(cli_hostname, cli_repo, cli_profile, number).await
        }
        MilestoneCommand::Create {
            title,
            description,
            due_on,
        } => {
            create(
                cli_hostname,
                cli_repo,
                cli_profile,
                title,
                description,
                due_on,
            )
            .await
        }
        MilestoneCommand::Edit {
            number,
            title,
            description,
            due_on,
            state,
        } => {
            edit(
                cli_hostname,
                cli_repo,
                cli_profile,
                number,
                title,
                description,
                due_on,
                state,
            )
            .await
        }
        MilestoneCommand::Delete { number, yes } => {
            delete(cli_hostname, cli_repo, cli_profile, number, yes).await
        }
    }
}

async fn list(
    hostname: &Option<String>,
    cli_repo: &Option<String>,
    cli_profile: &Option<String>,
    state: &str,
    json: bool,
) -> Result<()> {
    let hostname = resolve_hostname(hostname, cli_profile)?;
    let (owner, repo) = resolve_repo(cli_repo, cli_profile)?;
    let client = create_client(&hostname, cli_profile)?;
    let state = normalize_list_state(state)?;
    let milestones = client.list_milestones(&owner, &repo, &state).await?;

    if json {
        println!("{}", serde_json::to_string_pretty(&milestones)?);
        return Ok(());
    }

    let rows: Vec<Vec<String>> = milestones
        .iter()
        .map(|milestone| {
            vec![
                format!("#{}", milestone.number),
                format_state(&milestone.state),
                truncate(&milestone.title, 40),
                format_due_on(milestone.due_on.as_deref()),
                milestone.open_issues.unwrap_or(0).to_string(),
                milestone.closed_issues.unwrap_or(0).to_string(),
            ]
        })
        .collect();

    print_table(&["#", "STATE", "TITLE", "DUE", "OPEN", "CLOSED"], &rows);
    Ok(())
}

async fn view(
    hostname: &Option<String>,
    cli_repo: &Option<String>,
    cli_profile: &Option<String>,
    number: u64,
) -> Result<()> {
    let hostname = resolve_hostname(hostname, cli_profile)?;
    let (owner, repo) = resolve_repo(cli_repo, cli_profile)?;
    let client = create_client(&hostname, cli_profile)?;
    let milestone = client.get_milestone(&owner, &repo, number).await?;

    println!(
        "{} {}",
        milestone.title.bold(),
        format!("#{}", milestone.number).dimmed()
    );
    println!("{}", format_state(&milestone.state));
    println!();

    let due_on = format_due_on(milestone.due_on.as_deref());
    if !due_on.is_empty() {
        println!("Due: {}", due_on);
    }
    println!(
        "Open issues: {}  Closed issues: {}",
        milestone.open_issues.unwrap_or(0),
        milestone.closed_issues.unwrap_or(0)
    );

    if let Some(description) = milestone.description.as_deref() {
        if !description.is_empty() {
            println!();
            println!("{}", description);
        }
    }

    if let Some(url) = milestone.html_url.as_deref() {
        println!();
        println!("URL: {}", url);
    }

    Ok(())
}

async fn create(
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
async fn edit(
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
    let state = normalize_edit_state(state)?;
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

async fn delete(
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

fn normalize_due_on_for_create(raw: Option<String>) -> Result<Option<NormalizedDueOn>> {
    match raw {
        Some(value) if value.trim().is_empty() => Ok(None),
        Some(value) => Ok(Some(parse_due_on_value(&value)?)),
        None => Ok(None),
    }
}

fn normalize_due_on_for_edit(raw: Option<String>) -> Result<DueOnInput> {
    match raw {
        Some(value) if value.trim().is_empty() => Ok(DueOnInput::Clear),
        Some(value) => Ok(DueOnInput::Set(parse_due_on_value(&value)?)),
        None => Ok(DueOnInput::Unchanged),
    }
}

fn parse_due_on_value(value: &str) -> Result<NormalizedDueOn> {
    static DATE_FORMAT: &[time::format_description::FormatItem<'static>] =
        format_description!("[year]-[month]-[day]");

    let trimmed = value.trim();

    if let Ok(date) = Date::parse(trimmed, DATE_FORMAT) {
        let form_value = date
            .format(DATE_FORMAT)
            .map_err(|err| GbError::Other(format!("Failed to format due date: {}", err)))?;
        return Ok(NormalizedDueOn {
            api_value: format!("{form_value}T00:00:00Z"),
            form_value,
        });
    }

    if let Ok(date_time) = OffsetDateTime::parse(trimmed, &Rfc3339) {
        let form_value = date_time
            .date()
            .format(DATE_FORMAT)
            .map_err(|err| GbError::Other(format!("Failed to format due date: {}", err)))?;
        return Ok(NormalizedDueOn {
            api_value: format!("{form_value}T00:00:00Z"),
            form_value,
        });
    }

    Err(GbError::Other(format!(
        "Invalid due date '{}'. Expected YYYY-MM-DD or RFC3339.",
        value
    )))
}

fn due_on_to_form_date(value: &str) -> Result<String> {
    if value.starts_with("0001-01-01") {
        return Ok(String::new());
    }
    Ok(parse_due_on_value(value)?.form_value)
}

fn normalize_edit_state(state: Option<String>) -> Result<Option<String>> {
    match state {
        Some(state) => match state.to_ascii_lowercase().as_str() {
            "open" | "closed" => Ok(Some(state.to_ascii_lowercase())),
            _ => Err(GbError::Other(format!(
                "Invalid state '{}'. Expected one of: open, closed",
                state
            ))),
        },
        None => Ok(None),
    }
}

fn format_due_on(value: Option<&str>) -> String {
    match value {
        Some(v) if v.starts_with("0001-01-01") => String::new(),
        Some(v) => v.to_string(),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        normalize_due_on_for_create, normalize_due_on_for_edit, normalize_edit_state, DueOnInput,
    };

    #[test]
    fn create_due_on_accepts_plain_date() {
        let due_on = normalize_due_on_for_create(Some("2026-04-01".into())).unwrap();
        let due_on = due_on.unwrap();
        assert_eq!(due_on.api_value, "2026-04-01T00:00:00Z");
        assert_eq!(due_on.form_value, "2026-04-01");
    }

    #[test]
    fn create_due_on_accepts_rfc3339() {
        let due_on = normalize_due_on_for_create(Some("2026-04-01T09:30:00Z".into())).unwrap();
        let due_on = due_on.unwrap();
        assert_eq!(due_on.api_value, "2026-04-01T00:00:00Z");
        assert_eq!(due_on.form_value, "2026-04-01");
    }

    #[test]
    fn edit_due_on_empty_string_clears_value() {
        assert!(matches!(
            normalize_due_on_for_edit(Some(String::new())).unwrap(),
            DueOnInput::Clear
        ));
    }

    #[test]
    fn due_on_rejects_invalid_values() {
        assert!(normalize_due_on_for_create(Some("not-a-date".into())).is_err());
    }

    #[test]
    fn format_due_on_hides_unset_sentinel() {
        assert_eq!(super::format_due_on(Some("0001-01-01T00:00:00Z")), "");
        assert_eq!(
            super::format_due_on(Some("2026-04-01T00:00:00Z")),
            "2026-04-01T00:00:00Z"
        );
    }

    #[test]
    fn due_on_to_form_date_hides_unset_sentinel() {
        assert_eq!(
            super::due_on_to_form_date("0001-01-01T00:00:00Z").unwrap(),
            ""
        );
        assert_eq!(
            super::due_on_to_form_date("2026-04-01T00:00:00Z").unwrap(),
            "2026-04-01"
        );
    }

    #[test]
    fn normalize_edit_state_rejects_all() {
        assert!(normalize_edit_state(Some("all".into())).is_err());
    }
}
