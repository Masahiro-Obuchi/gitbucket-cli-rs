use colored::Colorize;

use crate::cli::common::{HostContext, RepoContext};
use crate::error::Result;
use crate::output;
use crate::output::table::print_table;
use crate::output::truncate;

use super::git::accessible_clone_url;

pub(super) async fn list(
    hostname: &Option<String>,
    cli_profile: &Option<String>,
    owner: Option<String>,
    json: bool,
) -> Result<()> {
    let ctx = HostContext::resolve(hostname, cli_profile)?;

    let repos = match owner {
        Some(ref o) => ctx.client.list_owner_repos(o).await?,
        None => ctx.client.list_my_repos().await?,
    };

    if json {
        return output::print_json(&repos);
    }

    let rows: Vec<Vec<String>> = repos
        .iter()
        .map(|r| {
            let visibility = if r.is_private {
                "private".yellow().to_string()
            } else {
                "public".green().to_string()
            };
            let desc = r.description.as_deref().unwrap_or("").to_string();
            vec![r.full_name.clone(), truncate(&desc, 50), visibility]
        })
        .collect();

    print_table(&["NAME", "DESCRIPTION", "VISIBILITY"], &rows);
    Ok(())
}

pub(super) async fn view(
    hostname: &Option<String>,
    repo_arg: Option<String>,
    cli_profile: &Option<String>,
    web: bool,
) -> Result<()> {
    let ctx = RepoContext::resolve(hostname, &repo_arg, cli_profile)?;

    if web {
        let url = ctx.client.web_url(&format!("/{}/{}", ctx.owner, ctx.repo));
        return output::open_web_url(&url);
    }

    let r = ctx.client.get_repo(&ctx.owner, &ctx.repo).await?;

    println!("{}", r.full_name.bold());
    if let Some(desc) = &r.description {
        if !desc.is_empty() {
            println!("{}", desc);
        }
    }
    println!();

    let visibility = if r.is_private { "Private" } else { "Public" };
    println!(
        "{}  {}  {}",
        format!("Visibility: {}", visibility).dimmed(),
        format!(
            "Default branch: {}",
            r.default_branch.as_deref().unwrap_or("main")
        )
        .dimmed(),
        if r.fork {
            "(fork)".dimmed().to_string()
        } else {
            String::new()
        },
    );

    if let Some(url) = &r.html_url {
        println!("URL: {}", url);
    }
    if let Some(url) = &r.clone_url {
        let fallback_clone_url = ctx
            .client
            .web_url(&format!("/{}/{}.git", ctx.owner, ctx.repo));
        println!(
            "Clone: {}",
            accessible_clone_url(Some(url), &fallback_clone_url)
        );
    }

    println!(
        "
Stars: {}  Forks: {}  Issues: {}",
        r.watchers_count.unwrap_or(0),
        r.forks_count.unwrap_or(0),
        r.open_issues_count.unwrap_or(0),
    );

    Ok(())
}
