use clap::{Args, Subcommand};

use crate::error::Result;

mod git;
mod read;
mod write;

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
        /// Target repository in OWNER/REPO format
        #[arg(long = "repo", short = 'R', value_name = "OWNER/REPO")]
        target_repo: Option<String>,
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
        /// Target repository in OWNER/REPO format
        #[arg(long = "repo", short = 'R', value_name = "OWNER/REPO")]
        target_repo: Option<String>,
        /// Skip confirmation
        #[arg(long)]
        yes: bool,
        /// Repository in OWNER/REPO format (or -R/--repo)
        #[arg(value_name = "OWNER/REPO")]
        repo: Option<String>,
    },
    /// Fork a repository
    Fork {
        /// Target repository in OWNER/REPO format
        #[arg(long = "repo", short = 'R', value_name = "OWNER/REPO")]
        target_repo: Option<String>,
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
    cli_profile: &Option<String>,
) -> Result<()> {
    match args.command {
        RepoCommand::List { owner, json } => {
            read::list(cli_hostname, cli_profile, owner, json).await
        }
        RepoCommand::View {
            target_repo,
            repo,
            web,
        } => {
            read::view(
                cli_hostname,
                repo.or(target_repo).or(cli_repo.clone()),
                cli_profile,
                web,
            )
            .await
        }
        RepoCommand::Create {
            name,
            description,
            private,
            add_readme,
            group,
        } => {
            write::create(
                cli_hostname,
                cli_profile,
                name,
                description,
                private,
                add_readme,
                group,
            )
            .await
        }
        RepoCommand::Clone { repo, directory } => {
            git::clone(cli_hostname, cli_profile, &repo, directory.as_deref()).await
        }
        RepoCommand::Delete {
            target_repo,
            repo,
            yes,
        } => {
            write::delete(
                cli_hostname,
                cli_profile,
                repo.or(target_repo).or(cli_repo.clone()),
                yes,
            )
            .await
        }
        RepoCommand::Fork {
            target_repo,
            repo,
            group,
        } => {
            write::fork(
                cli_hostname,
                repo.or(target_repo).or(cli_repo.clone()),
                cli_profile,
                group,
            )
            .await
        }
    }
}
