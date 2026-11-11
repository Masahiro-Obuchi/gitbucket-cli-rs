use clap::{Args, Subcommand};
use colored::Colorize;
use dialoguer::{Confirm, Input};

use crate::api::label::validate_label_segment;
use crate::cli::common::{create_web_session, RepoContext};
use crate::error::{GbError, Result};
use crate::models::label::{CreateLabel, UpdateLabel};
use crate::output;
use crate::output::table::print_table;
use crate::output::truncate;

#[derive(Args)]
pub struct LabelArgs {
    #[command(subcommand)]
    pub command: LabelCommand,
}

struct LabelEditRequest {
    new_name: Option<String>,
    color: Option<String>,
    description: Option<String>,
    remove_description: bool,
    web: bool,
}

#[derive(Subcommand)]
pub enum LabelCommand {
    /// List labels
    List {
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
    /// View a label
    View {
        /// Label name
        name: String,
    },
    /// Create a new label
    Create {
        /// Label name (prompts when omitted)
        name: Option<String>,
        /// Label color as 6-digit hex, with or without '#'
        #[arg(long, short)]
        color: Option<String>,
        /// Label description (unsupported on verified GitBucket versions)
        #[arg(long, short)]
        description: Option<String>,
    },
    /// Edit a label
    #[command(
        after_help = "Rename uses REST and retains the stored color. Same-name color changes may require --web on GitBucket 4.44; the numeric label ID is preserved. Description writes are unsupported and rejected before changes."
    )]
    Edit {
        /// Current label name
        name: String,
        /// New label name
        #[arg(long = "name")]
        new_name: Option<String>,
        /// New label color as 6-digit hex, with or without '#'
        #[arg(long, short)]
        color: Option<String>,
        /// Label description (unsupported on verified GitBucket versions)
        #[arg(long, short, conflicts_with = "remove_description")]
        description: Option<String>,
        /// Permit a verified Web color update if same-name REST PATCH is rejected
        #[arg(long)]
        web: bool,
        /// Clear the label description (unsupported on verified GitBucket versions)
        #[arg(long)]
        remove_description: bool,
    },
    /// Delete a label
    Delete {
        /// Label name
        name: String,
        /// Skip confirmation
        #[arg(long)]
        yes: bool,
    },
}

pub async fn run(
    args: LabelArgs,
    cli_hostname: &Option<String>,
    cli_repo: &Option<String>,
    cli_profile: &Option<String>,
) -> Result<()> {
    match args.command {
        LabelCommand::List { json } => list(cli_hostname, cli_repo, cli_profile, json).await,
        LabelCommand::View { name } => view(cli_hostname, cli_repo, cli_profile, &name).await,
        LabelCommand::Create {
            name,
            color,
            description,
        } => {
            create(
                cli_hostname,
                cli_repo,
                cli_profile,
                name,
                color,
                description,
            )
            .await
        }
        LabelCommand::Edit {
            name,
            new_name,
            color,
            description,
            remove_description,
            web,
        } => {
            edit(
                cli_hostname,
                cli_repo,
                cli_profile,
                &name,
                LabelEditRequest {
                    new_name,
                    color,
                    description,
                    remove_description,
                    web,
                },
            )
            .await
        }
        LabelCommand::Delete { name, yes } => {
            delete(cli_hostname, cli_repo, cli_profile, &name, yes).await
        }
    }
}

async fn list(
    hostname: &Option<String>,
    cli_repo: &Option<String>,
    cli_profile: &Option<String>,
    json: bool,
) -> Result<()> {
    let ctx = RepoContext::resolve(hostname, cli_repo, cli_profile)?;
    let labels = ctx.client.list_labels(&ctx.owner, &ctx.repo).await?;

    if json {
        return output::print_json(&labels);
    }

    let rows: Vec<Vec<String>> = labels
        .iter()
        .map(|label| {
            vec![
                label.name.clone(),
                label.color.as_deref().map(format_color).unwrap_or_default(),
                truncate(label.description.as_deref().unwrap_or(""), 60),
            ]
        })
        .collect();
    print_table(&["NAME", "COLOR", "DESCRIPTION"], &rows);
    Ok(())
}

async fn view(
    hostname: &Option<String>,
    cli_repo: &Option<String>,
    cli_profile: &Option<String>,
    name: &str,
) -> Result<()> {
    let ctx = RepoContext::resolve(hostname, cli_repo, cli_profile)?;
    let label = ctx.client.get_label(&ctx.owner, &ctx.repo, name).await?;

    println!("{}", label.name.bold());
    if let Some(color) = label.color.as_deref() {
        println!("Color: {}", format_color(color).dimmed());
    }
    if let Some(description) = label.description.as_deref() {
        if !description.is_empty() {
            println!("Description: {}", description);
        }
    }
    if let Some(url) = label.url.as_deref() {
        println!("API URL: {}", url);
    }

    Ok(())
}

async fn create(
    hostname: &Option<String>,
    cli_repo: &Option<String>,
    cli_profile: &Option<String>,
    name: Option<String>,
    color: Option<String>,
    description: Option<String>,
) -> Result<()> {
    if description.is_some() {
        return Err(description_unsupported());
    }
    let ctx = RepoContext::resolve(hostname, cli_repo, cli_profile)?;

    let name = match name {
        Some(name) => name,
        None => {
            crate::cancel::prompt(|| Input::new().with_prompt("Label name").interact_text()).await?
        }
    };

    validate_label_name(&name)?;
    let raw_color = match color {
        Some(color) => color,
        None => {
            crate::cancel::prompt(|| {
                Input::new()
                    .with_prompt("Label color (6-digit hex)")
                    .interact_text()
            })
            .await?
        }
    };
    let color = normalize_label_color(&raw_color)?;

    let label = ctx
        .client
        .create_label(
            &ctx.owner,
            &ctx.repo,
            &CreateLabel {
                name: name.clone(),
                color: color.clone(),
            },
        )
        .await?;

    verify_label(&label, &name, &color, None)?;
    let stored = ctx
        .client
        .get_label(&ctx.owner, &ctx.repo, &name)
        .await
        .map_err(|err| after_submission(err, "Label create"))?;
    verify_label(&stored, &name, &color, None)?;
    println!("✓ Created label {}", label.name);
    if let Some(color) = label.color.as_deref() {
        println!("Color: {}", format_color(color));
    }
    Ok(())
}

async fn edit(
    hostname: &Option<String>,
    cli_repo: &Option<String>,
    cli_profile: &Option<String>,
    name: &str,
    request: LabelEditRequest,
) -> Result<()> {
    if request.new_name.is_none()
        && request.color.is_none()
        && request.description.is_none()
        && !request.remove_description
    {
        return Err(GbError::Other(
            "No label changes requested. Pass at least one of --name, --color, --description, or --remove-description."
                .into(),
        ));
    }

    if request.description.is_some() || request.remove_description {
        return Err(description_unsupported());
    }
    validate_label_segment(name)?;
    if let Some(new_name) = &request.new_name {
        validate_label_name(new_name)?;
    }
    let requested_color = request
        .color
        .as_deref()
        .map(normalize_label_color)
        .transpose()?;
    let ctx = RepoContext::resolve(hostname, cli_repo, cli_profile)?;
    let current = ctx.client.get_label(&ctx.owner, &ctx.repo, name).await?;
    if current.name != name {
        return Err(GbError::Other(
            "Label lookup returned a different name. No changes were applied.".into(),
        ));
    }
    let old_color = current
        .color
        .as_deref()
        .map(normalize_label_color)
        .transpose()?;
    let new_name = request.new_name.unwrap_or_else(|| current.name.clone());
    let color = requested_color
        .or_else(|| old_color.clone())
        .ok_or_else(|| {
            GbError::Other(
                "Cannot preserve an unknown label color. Specify --color explicitly.".into(),
            )
        })?;
    let label = if new_name == current.name && old_color.as_deref() == Some(&color) {
        current.clone()
    } else {
        let response = ctx
            .client
            .update_label(
                &ctx.owner,
                &ctx.repo,
                name,
                &UpdateLabel {
                    name: new_name.clone(),
                    color: color.clone(),
                    description: current.description.clone(),
                },
            )
            .await;
        let mut stored_after_web = None;
        match response {
            Ok(label) => {
                verify_label(&label, &new_name, &color, current.description.as_deref())?;
            }
            Err(GbError::Api { status: 422, .. }) if new_name == current.name => {
                if !request.web {
                    return Err(GbError::Other("The same-name REST label update was rejected (GitBucket 4.44 limitation). Re-run with --web to permit an in-place color update; the label will not be deleted or recreated.".into()));
                }
                validate_web_label(&current)?;
                let old_color = old_color.as_deref().ok_or_else(|| GbError::Other("Cannot verify the Web form without the current label color. No Web mutation was attempted.".into()))?;
                eprintln!("Notice: Same-name REST label update was rejected; using the verified Web color route in place.");
                let mut progress = crate::operation::Progress::default();
                let step = progress.plan("label.color", crate::operation::Route::Web);
                let session = create_web_session(&ctx.client, &ctx.hostname, cli_profile)
                    .await
                    .map_err(|err| progress.error(err))?;
                progress.actor(session.principal());
                let target = session
                    .prepare_label_color(&ctx.owner, &ctx.repo, name, old_color)
                    .await
                    .map_err(|err| progress.error(err))?;
                stored_after_web = Some(
                    progress
                        .run(
                            step,
                            session.update_label_color(&target, &color),
                            |_| async {
                                let stored = ctx
                                    .client
                                    .get_label(&ctx.owner, &ctx.repo, &new_name)
                                    .await?;
                                verify_label(
                                    &stored,
                                    &new_name,
                                    &color,
                                    current.description.as_deref(),
                                )?;
                                Ok(stored)
                            },
                        )
                        .await?,
                );
            }
            Err(err) => return Err(err),
        }
        let stored = if let Some(stored) = stored_after_web {
            stored
        } else {
            ctx.client
                .get_label(&ctx.owner, &ctx.repo, &new_name)
                .await
                .map_err(|err| after_submission(err, "Label update"))?
        };
        verify_label(&stored, &new_name, &color, current.description.as_deref())?;
        stored
    };

    println!("✓ Updated label {}", label.name);
    if let Some(color) = label.color.as_deref() {
        println!("Color: {}", format_color(color));
    }
    if let Some(description) = label.description.as_deref() {
        if !description.is_empty() {
            println!("Description: {}", description);
        }
    }
    Ok(())
}

async fn delete(
    hostname: &Option<String>,
    cli_repo: &Option<String>,
    cli_profile: &Option<String>,
    name: &str,
    yes: bool,
) -> Result<()> {
    let ctx = RepoContext::resolve(hostname, cli_repo, cli_profile)?;

    if !yes {
        let prompt = format!(
            "Are you sure you want to delete label '{}' from {}/{}?",
            name, ctx.owner, ctx.repo
        );
        let confirmed = crate::cancel::prompt(move || {
            Confirm::new().with_prompt(prompt).default(false).interact()
        })
        .await?;
        if !confirmed {
            println!("Aborted.");
            return Ok(());
        }
    }

    ctx.client.delete_label(&ctx.owner, &ctx.repo, name).await?;
    println!("✓ Deleted label {}", name);
    Ok(())
}

fn description_unsupported() -> GbError {
    GbError::Other("Label description writes are unsupported by the verified GitBucket 4.44 API and Web contracts; this CLI cannot confirm support on other instances. No changes were applied.".into())
}

fn validate_label_name(name: &str) -> Result<()> {
    validate_label_segment(name)?;
    if name.trim_matches(|ch| ch <= '\u{20}').is_empty()
        || name.encode_utf16().count() > 100
        || name.starts_with(['_', '-'])
    {
        return Err(GbError::Other("Label names must be nonblank, at most 100 UTF-16 units, and cannot start with '_' or '-'.".into()));
    }
    Ok(())
}

fn validate_web_label(current: &crate::models::label::Label) -> Result<()> {
    validate_label_name(&current.name)?;
    if current.name.trim_matches(|ch| ch <= '\u{20}') != current.name
        || current.name.contains(',')
        || current
            .description
            .as_deref()
            .is_some_and(|value| !value.is_empty())
    {
        return Err(GbError::Other("The verified Web label form cannot preserve this name or description. No Web mutation was attempted; use a server-side REST fix.".into()));
    }
    Ok(())
}

fn after_submission(err: GbError, action: &str) -> GbError {
    GbError::Other(format!(
        "{action} was submitted, but verification failed: {err}. Inspect labels before retrying."
    ))
}

fn verify_label(
    label: &crate::models::label::Label,
    name: &str,
    color: &str,
    description: Option<&str>,
) -> Result<()> {
    if label.name != name
        || label
            .color
            .as_deref()
            .map(normalize_label_color)
            .transpose()?
            .as_deref()
            != Some(color)
        || label.description.as_deref().unwrap_or_default() != description.unwrap_or_default()
    {
        return Err(GbError::Other("Label mutation was submitted, but stored name/color/description does not match. Inspect labels and Issue associations before retrying.".into()));
    }
    Ok(())
}

fn normalize_label_color(value: &str) -> Result<String> {
    let trimmed = value.trim().trim_start_matches('#');
    if trimmed.len() != 6 || !trimmed.chars().all(|ch| ch.is_ascii_hexdigit()) {
        return Err(GbError::Other(format!(
            "Invalid label color '{}'. Expected a 6-digit hex value like ff0000.",
            value
        )));
    }
    Ok(trimmed.to_ascii_lowercase())
}

fn format_color(color: &str) -> String {
    format!("#{}", color.trim_start_matches('#'))
}

#[cfg(test)]
mod tests {
    use super::{normalize_label_color, validate_web_label};

    #[test]
    fn web_label_validation_preserves_non_ascii_spaces_like_java_trim() {
        let label =
            serde_json::from_value(serde_json::json!({"name":"\u{a0}", "color":"123abc"})).unwrap();
        validate_web_label(&label).unwrap();
    }

    #[test]
    fn normalize_label_color_accepts_hash_prefix() {
        assert_eq!(normalize_label_color("#A1B2C3").unwrap(), "a1b2c3");
    }

    #[test]
    fn normalize_label_color_rejects_invalid_values() {
        assert!(normalize_label_color("zzz").is_err());
        assert!(normalize_label_color("12345").is_err());
    }
}
