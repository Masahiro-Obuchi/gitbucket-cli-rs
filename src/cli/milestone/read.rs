use colored::Colorize;

use crate::cli::common::{create_client, normalize_list_state, resolve_hostname, resolve_repo};
use crate::error::Result;
use crate::output::table::print_table;
use crate::output::{format_state, truncate};

use super::due_date::format_due_on;

pub(super) async fn list(
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

pub(super) async fn view(
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

    if let Some(description) = milestone
        .description
        .as_deref()
        .filter(|description| !description.is_empty())
    {
        println!();
        println!("{}", description);
    }

    if let Some(url) = milestone.html_url.as_deref() {
        println!();
        println!("URL: {}", url);
    }

    Ok(())
}
