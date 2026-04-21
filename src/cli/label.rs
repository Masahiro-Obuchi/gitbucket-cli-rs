use clap::{Args, Subcommand};
use colored::Colorize;
use dialoguer::{Confirm, Input};

use crate::cli::common::{create_client, resolve_hostname, resolve_repo};
use crate::error::{GbError, Result};
use crate::models::label::CreateLabel;
use crate::output::table::print_table;
use crate::output::truncate;

#[derive(Args)]
pub struct LabelArgs {
    #[command(subcommand)]
    pub command: LabelCommand,
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
        /// Optional label description
        #[arg(long, short)]
        description: Option<String>,
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
    let hostname = resolve_hostname(hostname, cli_profile)?;
    let (owner, repo) = resolve_repo(cli_repo, cli_profile)?;
    let client = create_client(&hostname, cli_profile)?;
    let labels = client.list_labels(&owner, &repo).await?;

    if json {
        println!("{}", serde_json::to_string_pretty(&labels)?);
        return Ok(());
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
    let hostname = resolve_hostname(hostname, cli_profile)?;
    let (owner, repo) = resolve_repo(cli_repo, cli_profile)?;
    let client = create_client(&hostname, cli_profile)?;
    let label = client.get_label(&owner, &repo, name).await?;

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
    let hostname = resolve_hostname(hostname, cli_profile)?;
    let (owner, repo) = resolve_repo(cli_repo, cli_profile)?;
    let client = create_client(&hostname, cli_profile)?;

    let name = match name {
        Some(name) => name,
        None => Input::new().with_prompt("Label name").interact_text()?,
    };

    let raw_color = match color {
        Some(color) => color,
        None => Input::new()
            .with_prompt("Label color (6-digit hex)")
            .interact_text()?,
    };
    let color = normalize_label_color(&raw_color)?;

    let label = client
        .create_label(
            &owner,
            &repo,
            &CreateLabel {
                name: name.clone(),
                color,
                description,
            },
        )
        .await?;

    println!("✓ Created label {}", label.name);
    if let Some(color) = label.color.as_deref() {
        println!("Color: {}", format_color(color));
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
    let hostname = resolve_hostname(hostname, cli_profile)?;
    let (owner, repo) = resolve_repo(cli_repo, cli_profile)?;

    if !yes {
        let confirmed = Confirm::new()
            .with_prompt(format!(
                "Are you sure you want to delete label '{}' from {}/{}?",
                name, owner, repo
            ))
            .default(false)
            .interact()?;
        if !confirmed {
            println!("Aborted.");
            return Ok(());
        }
    }

    let client = create_client(&hostname, cli_profile)?;
    client.delete_label(&owner, &repo, name).await?;
    println!("✓ Deleted label {}", name);
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
    use super::normalize_label_color;

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
