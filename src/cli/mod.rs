pub mod api;
pub mod auth;
pub mod auth_status;
pub mod common;
pub mod completion;
pub mod config;
pub mod config_output;
mod editor;
pub mod issue;
pub mod issue_like;
mod json_output;
pub mod label;
pub mod milestone;
pub(crate) mod parse;
pub mod pr;
pub mod repo;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "gb",
    about = "GitBucket command-line client",
    after_long_help = "Use gb <command> -h for options. See docs/reference/automation.md for deadlines and automation.",
    version
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,

    /// GitBucket base URL or hostname
    #[arg(
        long,
        short = 'H',
        global = true,
        env = "GB_HOST",
        value_name = "HOST_OR_URL"
    )]
    pub hostname: Option<String>,

    /// Target OWNER/REPO
    #[arg(
        long,
        short = 'R',
        global = true,
        env = "GB_REPO",
        long_help = "Target OWNER/REPO. If no explicit/environment/profile repository is selected, origin must match the configured GitBucket host and deployment path; specify --repo to select a different target intentionally."
    )]
    pub repo: Option<String>,

    /// Configuration profile name
    #[arg(long, global = true, env = "GB_PROFILE", value_name = "NAME")]
    pub profile: Option<String>,

    /// Report errors as JSON and disable prompts
    #[arg(long, global = true, env = "GB_JSON_ERRORS", value_parser = clap::builder::BoolishValueParser::new(),
        long_help = "Print one JSON error on stderr: argument errors exit 2 and runtime errors exit 1. Help/version remain normal successful output. Interactive prompts are disabled; supply options and confirmations explicitly. Explicit stdin input remains available. Command failures take precedence over later stdout errors. Successful output is unchanged; use --json where supported to select JSON data.")]
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

pub(crate) mod input;
