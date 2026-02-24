use clap::{Args, Subcommand};
use colored::Colorize;
use dialoguer::{Confirm, Input};

use crate::cli::common::{create_client, resolve_hostname, resolve_repo};
use crate::error::Result;
use crate::models::repository::CreateRepository;
use crate::output::table::print_table;
use crate::output::truncate;

#[derive(Args)]
pub struct RepoArgs {
    #[command(subcommand)]
    pub command: RepoCommand,
}

#[derive(Subcommand)]
pub enum RepoCommand {
    /// List repositories
    List {
        /// Owner (user or organization). If omitted, lists your repositories.
        owner: Option<String>,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
    /// View repository details
    View {
        /// Repository in OWNER/REPO format
        repo: Option<String>,
        /// Open in browser
        #[arg(long, short)]
        web: bool,
    },
    /// Create a new repository
    Create {
        /// Repository name
        name: Option<String>,
        /// Description
        #[arg(long, short)]
        description: Option<String>,
        /// Make the repository private
        #[arg(long)]
        private: bool,
        /// Initialize with a README
        #[arg(long)]
        add_readme: bool,
        /// Organization to create under
        #[arg(long)]
        org: Option<String>,
    },
    /// Clone a repository
    Clone {
        /// Repository to clone (OWNER/REPO or full URL)
        repo: String,
        /// Directory to clone into
        directory: Option<String>,
    },
    /// Delete a repository
    Delete {
        /// Repository in OWNER/REPO format
        repo: Option<String>,
        /// Skip confirmation
        #[arg(long)]
        yes: bool,
    },
    /// Fork a repository
    Fork {
        /// Repository to fork (OWNER/REPO)
        repo: Option<String>,
    },
}

pub async fn run(
    args: RepoArgs,
    cli_hostname: &Option<String>,
    cli_repo: &Option<String>,
) -> Result<()> {
    match args.command {
        RepoCommand::List { owner, json } => list(cli_hostname, owner, json).await,
        RepoCommand::View { repo, web } => {
            view(cli_hostname, repo.as_ref().or(cli_repo.as_ref()).cloned(), web).await
        }
        RepoCommand::Create {
            name,
            description,
            private,
            add_readme,
            org,
        } => create(cli_hostname, name, description, private, add_readme, org).await,
        RepoCommand::Clone { repo, directory } => {
            clone(cli_hostname, &repo, directory.as_deref()).await
        }
        RepoCommand::Delete { repo, yes } => {
            delete(
                cli_hostname,
                repo.as_ref().or(cli_repo.as_ref()).cloned(),
                yes,
            )
            .await
        }
        RepoCommand::Fork { repo } => {
            fork(cli_hostname, repo.as_ref().or(cli_repo.as_ref()).cloned()).await
        }
    }
}

async fn list(hostname: &Option<String>, owner: Option<String>, json: bool) -> Result<()> {
    let hostname = resolve_hostname(hostname)?;
    let client = create_client(&hostname)?;

    let repos = match owner {
        Some(ref o) => client.list_user_repos(o).await?,
        None => client.list_my_repos().await?,
    };

    if json {
        println!("{}", serde_json::to_string_pretty(&repos)?);
        return Ok(());
    }

    let rows: Vec<Vec<String>> = repos
        .iter()
        .map(|r| {
            let visibility = if r.is_private {
                "private".yellow().to_string()
            } else {
                "public".green().to_string()
            };
            let desc = r
                .description
                .as_deref()
                .unwrap_or("")
                .to_string();
            vec![
                r.full_name.clone(),
                truncate(&desc, 50),
                visibility,
            ]
        })
        .collect();

    print_table(&["NAME", "DESCRIPTION", "VISIBILITY"], &rows);
    Ok(())
}

async fn view(hostname: &Option<String>, repo_arg: Option<String>, web: bool) -> Result<()> {
    let hostname = resolve_hostname(hostname)?;
    let (owner, repo) = match repo_arg {
        Some(r) => crate::cli::common::parse_owner_repo(&r)?,
        None => resolve_repo(&None)?,
    };
    let client = create_client(&hostname)?;

    if web {
        let url = client.web_url(&format!("/{}/{}", owner, repo));
        open::that(&url).map_err(|e| crate::error::GbError::Other(format!("Failed to open browser: {}", e)))?;
        println!("Opening {} in your browser.", url);
        return Ok(());
    }

    let r = client.get_repo(&owner, &repo).await?;

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
        format!("Default branch: {}", r.default_branch.as_deref().unwrap_or("main")).dimmed(),
        if r.fork { "(fork)".dimmed().to_string() } else { String::new() },
    );

    if let Some(url) = &r.html_url {
        println!("URL: {}", url);
    }
    if let Some(url) = &r.clone_url {
        println!("Clone: {}", url);
    }

    println!(
        "\nStars: {}  Forks: {}  Issues: {}",
        r.watchers_count.unwrap_or(0),
        r.forks_count.unwrap_or(0),
        r.open_issues_count.unwrap_or(0),
    );

    Ok(())
}

async fn create(
    hostname: &Option<String>,
    name: Option<String>,
    description: Option<String>,
    private: bool,
    add_readme: bool,
    org: Option<String>,
) -> Result<()> {
    let hostname = resolve_hostname(hostname)?;
    let client = create_client(&hostname)?;

    let name = match name {
        Some(n) => n,
        None => Input::new()
            .with_prompt("Repository name")
            .interact_text()?,
    };

    let body = CreateRepository {
        name: name.clone(),
        description,
        is_private: Some(private),
        auto_init: Some(add_readme),
    };

    let repo = match org {
        Some(o) => client.create_org_repo(&o, &body).await?,
        None => client.create_user_repo(&body).await?,
    };

    println!("✓ Created repository {}", repo.full_name);
    if let Some(url) = &repo.html_url {
        println!("{}", url);
    }
    Ok(())
}

async fn clone(hostname: &Option<String>, repo: &str, directory: Option<&str>) -> Result<()> {
    let hostname = resolve_hostname(hostname)?;
    let client = create_client(&hostname)?;

    let clone_url = if repo.contains("://") || repo.contains('@') {
        repo.to_string()
    } else {
        let (owner, name) = crate::cli::common::parse_owner_repo(repo)?;
        let r = client.get_repo(&owner, &name).await?;
        r.clone_url
            .unwrap_or_else(|| client.web_url(&format!("/{}/{}.git", owner, name)))
    };

    let mut cmd = std::process::Command::new("git");
    cmd.arg("clone").arg(&clone_url);
    if let Some(dir) = directory {
        cmd.arg(dir);
    }

    let status = cmd.status()?;
    if !status.success() {
        return Err(crate::error::GbError::Other("git clone failed".into()));
    }

    Ok(())
}

async fn delete(
    hostname: &Option<String>,
    repo_arg: Option<String>,
    yes: bool,
) -> Result<()> {
    let hostname = resolve_hostname(hostname)?;
    let (owner, repo) = match repo_arg {
        Some(r) => crate::cli::common::parse_owner_repo(&r)?,
        None => resolve_repo(&None)?,
    };

    if !yes {
        let confirmed = Confirm::new()
            .with_prompt(format!(
                "Are you sure you want to delete {}/{}?",
                owner, repo
            ))
            .default(false)
            .interact()?;
        if !confirmed {
            println!("Aborted.");
            return Ok(());
        }
    }

    let client = create_client(&hostname)?;
    client.delete_repo(&owner, &repo).await?;
    println!("✓ Deleted repository {}/{}", owner, repo);
    Ok(())
}

async fn fork(hostname: &Option<String>, repo_arg: Option<String>) -> Result<()> {
    let hostname = resolve_hostname(hostname)?;
    let (owner, repo) = match repo_arg {
        Some(r) => crate::cli::common::parse_owner_repo(&r)?,
        None => resolve_repo(&None)?,
    };

    let client = create_client(&hostname)?;
    let forked = client.fork_repo(&owner, &repo).await?;
    println!("✓ Forked {}/{} → {}", owner, repo, forked.full_name);
    if let Some(url) = &forked.html_url {
        println!("{}", url);
    }
    Ok(())
}
