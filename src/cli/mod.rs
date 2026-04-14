pub mod api;
pub mod auth;
pub mod common;
pub mod completion;
pub mod config;
pub mod issue;
pub mod label;
pub mod milestone;
pub mod pr;
pub mod repo;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "gb",
    about = "GitBucket CLI - Work seamlessly with GitBucket from the command line",
    after_help = "Run `gb <command> --help` for command-specific options.",
    version
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,

    /// GitBucket host or base URL
    #[arg(
        long,
        short = 'H',
        global = true,
        env = "GB_HOST",
        value_name = "HOST_OR_URL"
    )]
    pub hostname: Option<String>,

    /// Target repository in OWNER/REPO format
    #[arg(long, short = 'R', global = true, env = "GB_REPO")]
    pub repo: Option<String>,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Call the GitBucket REST API directly
    Api(api::ApiArgs),
    /// Authenticate with a GitBucket instance
    Auth(auth::AuthArgs),
    /// Work with repositories
    Repo(repo::RepoArgs),
    /// Manage local CLI configuration
    Config(config::ConfigArgs),
    /// Generate shell completion scripts
    Completion(completion::CompletionArgs),
    /// Work with issues
    Issue(issue::IssueArgs),
    /// Work with labels
    Label(label::LabelArgs),
    /// Work with milestones
    Milestone(milestone::MilestoneArgs),
    /// Work with pull requests
    Pr(pr::PrArgs),
    /// Open the repository in a web browser
    Browse,
}
