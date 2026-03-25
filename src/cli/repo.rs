use clap::{Args, Subcommand};
use colored::Colorize;
use dialoguer::{Confirm, Input};

use crate::cli::common::{
    create_client, create_web_session, parse_owner_repo, resolve_host_config, resolve_hostname,
    resolve_repo,
};
use crate::error::{GbError, Result};
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
        /// Owner (user or group). If omitted, lists your repositories.
        owner: Option<String>,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
    /// View repository details
    View {
        /// Repository in OWNER/REPO format
        repo: Option<String>,
        /// Repository in OWNER/REPO format
        #[arg(long = "repo", short = 'R', conflicts_with = "repo")]
        repo_flag: Option<String>,
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
        /// Group to create under
        #[arg(long = "group", alias = "org")]
        group: Option<String>,
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
        /// Repository in OWNER/REPO format
        #[arg(long = "repo", short = 'R', conflicts_with = "repo")]
        repo_flag: Option<String>,
        /// Skip confirmation
        #[arg(long)]
        yes: bool,
    },
    /// Fork a repository
    Fork {
        /// Repository to fork (OWNER/REPO)
        repo: Option<String>,
        /// Repository to fork (OWNER/REPO)
        #[arg(long = "repo", short = 'R', conflicts_with = "repo")]
        repo_flag: Option<String>,
        /// Group to fork into (defaults to your user)
        #[arg(long = "group", alias = "org")]
        group: Option<String>,
    },
}

pub async fn run(
    args: RepoArgs,
    cli_hostname: &Option<String>,
    cli_repo: &Option<String>,
) -> Result<()> {
    match args.command {
        RepoCommand::List { owner, json } => list(cli_hostname, owner, json).await,
        RepoCommand::View {
            repo,
            repo_flag,
            web,
        } => view(cli_hostname, repo.or(repo_flag).or(cli_repo.clone()), web).await,
        RepoCommand::Create {
            name,
            description,
            private,
            add_readme,
            group,
        } => create(cli_hostname, name, description, private, add_readme, group).await,
        RepoCommand::Clone { repo, directory } => {
            clone(cli_hostname, &repo, directory.as_deref()).await
        }
        RepoCommand::Delete {
            repo,
            repo_flag,
            yes,
        } => delete(cli_hostname, repo.or(repo_flag).or(cli_repo.clone()), yes).await,
        RepoCommand::Fork {
            repo,
            repo_flag,
            group,
        } => fork(cli_hostname, repo.or(repo_flag).or(cli_repo.clone()), group).await,
    }
}

async fn list(hostname: &Option<String>, owner: Option<String>, json: bool) -> Result<()> {
    let hostname = resolve_hostname(hostname)?;
    let client = create_client(&hostname)?;

    let repos = match owner {
        Some(ref o) => client.list_owner_repos(o).await?,
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
            let desc = r.description.as_deref().unwrap_or("").to_string();
            vec![r.full_name.clone(), truncate(&desc, 50), visibility]
        })
        .collect();

    print_table(&["NAME", "DESCRIPTION", "VISIBILITY"], &rows);
    Ok(())
}

async fn view(hostname: &Option<String>, repo_arg: Option<String>, web: bool) -> Result<()> {
    let hostname = resolve_hostname(hostname)?;
    let (owner, repo) = match repo_arg {
        Some(r) => parse_owner_repo(&r)?,
        None => resolve_repo(&None)?,
    };
    let client = create_client(&hostname)?;

    if web {
        let url = client.web_url(&format!("/{}/{}", owner, repo));
        open::that(&url)
            .map_err(|e| crate::error::GbError::Other(format!("Failed to open browser: {}", e)))?;
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
        println!("Clone: {}", url);
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

async fn create(
    hostname: &Option<String>,
    name: Option<String>,
    description: Option<String>,
    private: bool,
    add_readme: bool,
    group: Option<String>,
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

    let repo = match group {
        Some(group_name) => client.create_org_repo(&group_name, &body).await?,
        None => client.create_user_repo(&body).await?,
    };

    println!("✓ Created repository {}", repo.full_name);
    if let Some(url) = &repo.html_url {
        println!("{}", url);
    }
    Ok(())
}

async fn clone(hostname: &Option<String>, repo: &str, directory: Option<&str>) -> Result<()> {
    let clone_url = if repo.contains("://") || repo.contains('@') {
        repo.to_string()
    } else {
        let hostname = resolve_hostname(hostname)?;
        let client = create_client(&hostname)?;
        let (owner, name) = parse_owner_repo(repo)?;
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

async fn delete(hostname: &Option<String>, repo_arg: Option<String>, yes: bool) -> Result<()> {
    let hostname = resolve_hostname(hostname)?;
    let repo_arg = repo_arg.ok_or_else(|| {
        crate::error::GbError::Other(
            "Refusing to delete without an explicit repository. Pass OWNER/REPO or -R/--repo."
                .into(),
        )
    })?;
    let (owner, repo) = parse_owner_repo(&repo_arg)?;

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

async fn fork(
    hostname: &Option<String>,
    repo_arg: Option<String>,
    group: Option<String>,
) -> Result<()> {
    let hostname = resolve_hostname(hostname)?;
    let (owner, repo) = match repo_arg {
        Some(r) => parse_owner_repo(&r)?,
        None => resolve_repo(&None)?,
    };

    let client = create_client(&hostname)?;
    match client.fork_repo(&owner, &repo).await {
        Ok(forked) => {
            println!("✓ Forked {}/{} → {}", owner, repo, forked.full_name);
            if let Some(url) = &forked.html_url {
                println!("{}", url);
            }
            Ok(())
        }
        Err(GbError::Api { status: 404, .. }) => {
            let target_account = resolve_fork_target(&hostname, group)?;
            let session = create_web_session(&hostname).await?;
            session.fork_repo(&owner, &repo, &target_account).await?;
            println!("✓ Forked {}/{} → {}/{}", owner, repo, target_account, repo);
            println!(
                "{}",
                client.web_url(&format!("/{}/{}", target_account, repo))
            );
            Ok(())
        }
        Err(err) => Err(err),
    }
}

fn resolve_fork_target(hostname: &str, group: Option<String>) -> Result<String> {
    if let Some(group) = group {
        return Ok(group);
    }
    if let Ok(user) = std::env::var("GB_USER") {
        if !user.is_empty() {
            return Ok(user);
        }
    }

    let host = resolve_host_config(hostname)?;
    if !host.user.is_empty() {
        return Ok(host.user);
    }

    Err(GbError::Auth(
        "GitBucket fork requires a destination user or group. Run `gb auth login` first, pass `--group`, or set `GB_USER`."
            .into(),
    ))
}
