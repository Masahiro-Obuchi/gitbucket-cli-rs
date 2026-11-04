pub mod api;
pub mod auth;
pub mod auth_status;
pub mod common;
pub mod completion;
pub mod config;
pub mod config_output;
pub mod issue;
pub mod issue_like;
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
    #[arg(
        long,
        short = 'R',
        global = true,
        env = "GB_REPO",
        long_help = "Target repository in OWNER/REPO format. If no explicit/environment/profile repository is selected, origin must match the configured GitBucket host and deployment path; specify --repo to select a different target intentionally."
    )]
    pub repo: Option<String>,

    /// Configuration profile name
    #[arg(long, global = true, env = "GB_PROFILE", value_name = "NAME")]
    pub profile: Option<String>,

    /// Print failures as JSON on stderr, including available operation outcomes
    #[arg(long, global = true, env = "GB_JSON_ERRORS")]
    pub json_errors: bool,
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
