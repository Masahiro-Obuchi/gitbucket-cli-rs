use clap::{Args, Subcommand};
use colored::Colorize;
use dialoguer::{Confirm, Input};
use url::Url;

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
        #[arg(value_name = "OWNER")]
        owner: Option<String>,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
    /// View repository details
    View {
        /// Repository in OWNER/REPO format (defaults to -R or git remote)
        #[arg(value_name = "OWNER/REPO")]
        repo: Option<String>,
        /// Open in browser
        #[arg(long, short)]
        web: bool,
    },
    /// Create a new repository
    Create {
        /// Repository name (prompts when omitted)
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
        #[arg(long = "group", visible_alias = "org")]
        group: Option<String>,
    },
    /// Clone a repository
    Clone {
        /// Repository to clone (OWNER/REPO or full URL)
        #[arg(value_name = "OWNER/REPO|URL")]
        repo: String,
        /// Directory to clone into
        directory: Option<String>,
    },
    /// Delete a repository
    Delete {
        /// Skip confirmation
        #[arg(long)]
        yes: bool,
        /// Repository in OWNER/REPO format (or -R/--repo)
        #[arg(value_name = "OWNER/REPO")]
        repo: Option<String>,
    },
    /// Fork a repository
    Fork {
        /// Repository to fork (OWNER/REPO, or -R/--repo)
        #[arg(value_name = "OWNER/REPO")]
        repo: Option<String>,
        /// Group to fork into (defaults to your user)
        #[arg(long = "group", visible_alias = "org")]
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
        RepoCommand::View { repo, web } => view(cli_hostname, repo.or(cli_repo.clone()), web).await,
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
        RepoCommand::Delete { repo, yes } => {
            delete(cli_hostname, repo.or(cli_repo.clone()), yes).await
        }
        RepoCommand::Fork { repo, group } => {
            fork(cli_hostname, repo.or(cli_repo.clone()), group).await
        }
    }
}

fn public_repo_prefix(path: &str) -> String {
    let segments: Vec<&str> = path
        .split('/')
        .filter(|segment| !segment.is_empty())
        .collect();
    if segments.len() <= 2 {
        return String::new();
    }

    format!("/{}", segments[..segments.len() - 2].join("/"))
}

fn is_internal_gitbucket_clone_host(url: &Url) -> bool {
    matches!(url.host_str(), Some("gitbucket"))
}

fn same_origin(left: &Url, right: &Url) -> bool {
    left.scheme() == right.scheme()
        && left.host_str() == right.host_str()
        && left.port_or_known_default() == right.port_or_known_default()
}

fn accessible_clone_url(api_clone_url: Option<&str>, fallback_url: &str) -> String {
    let Some(api_clone_url) = api_clone_url else {
        return fallback_url.to_string();
    };

    let Ok(public_url) = Url::parse(fallback_url) else {
        return api_clone_url.to_string();
    };
    let Ok(api_url) = Url::parse(api_clone_url) else {
        return api_clone_url.to_string();
    };

    if !is_internal_gitbucket_clone_host(&api_url) && !same_origin(&api_url, &public_url) {
        return api_clone_url.to_string();
    }

    let public_prefix = public_repo_prefix(public_url.path());
    let mut clone_url = if is_internal_gitbucket_clone_host(&api_url) {
        public_url
    } else {
        api_url.clone()
    };
    let api_path = api_url.path();
    let normalized_api_path = if api_path.starts_with('/') {
        api_path.to_string()
    } else {
        format!("/{api_path}")
    };
    let combined_path = if public_prefix.is_empty()
        || normalized_api_path.starts_with(&format!("{public_prefix}/"))
    {
        normalized_api_path
    } else {
        format!("{public_prefix}{normalized_api_path}")
    };

    clone_url.set_path(&combined_path);
    clone_url.set_query(api_url.query());
    clone_url.set_fragment(api_url.fragment());
    clone_url.to_string()
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
        let fallback_clone_url = client.web_url(&format!("/{}/{}.git", owner, repo));
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
        let fallback_clone_url = client.web_url(&format!("/{}/{}.git", owner, name));
        accessible_clone_url(r.clone_url.as_deref(), &fallback_clone_url)
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
    match client.delete_repo(&owner, &repo).await {
        Ok(()) => {
            println!("✓ Deleted repository {}/{}", owner, repo);
            Ok(())
        }
        Err(GbError::Api { status: 404, .. }) => {
            let session = create_web_session(&hostname).await?;
            session.delete_repo(&owner, &repo).await?;
            println!("✓ Deleted repository {}/{}", owner, repo);
            Ok(())
        }
        Err(err) => Err(err),
    }
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

#[cfg(test)]
mod tests {
    use super::{accessible_clone_url, public_repo_prefix};

    #[test]
    fn public_repo_prefix_extracts_optional_base_path() {
        assert_eq!(public_repo_prefix("/alice/demo.git"), "");
        assert_eq!(
            public_repo_prefix("/gitbucket/alice/demo.git"),
            "/gitbucket"
        );
    }

    #[test]
    fn accessible_clone_url_rewrites_internal_host_to_public_base() {
        let rewritten = accessible_clone_url(
            Some("http://gitbucket:8080/git/alice/demo.git"),
            "http://127.0.0.1:18080/gitbucket/alice/demo.git",
        );

        assert_eq!(
            rewritten,
            "http://127.0.0.1:18080/gitbucket/git/alice/demo.git"
        );
    }

    #[test]
    fn accessible_clone_url_keeps_matching_public_clone_url() {
        let clone_url = "http://127.0.0.1:18080/gitbucket/git/alice/demo.git";
        let fallback_url = "http://127.0.0.1:18080/gitbucket/alice/demo.git";

        assert_eq!(
            accessible_clone_url(Some(clone_url), fallback_url),
            clone_url
        );
    }

    #[test]
    fn accessible_clone_url_preserves_external_clone_origin() {
        let clone_url = "https://clone.gitbucket.example.com/git/alice/demo.git";
        let fallback_url = "https://gitbucket.example.com/gitbucket/alice/demo.git";

        assert_eq!(
            accessible_clone_url(Some(clone_url), fallback_url),
            clone_url
        );
    }

    #[test]
    fn accessible_clone_url_falls_back_when_api_clone_url_is_missing() {
        let fallback_url = "http://127.0.0.1:18080/gitbucket/alice/demo.git";

        assert_eq!(accessible_clone_url(None, fallback_url), fallback_url);
    }
}
