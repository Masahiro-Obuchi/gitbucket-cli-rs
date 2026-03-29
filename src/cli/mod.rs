pub mod api;
pub mod auth;
pub mod common;
pub mod config;
pub mod issue;
pub mod label;
pub mod pr;
pub mod repo;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "gb",
    about = "GitBucket CLI - Work seamlessly with GitBucket from the command line",
    version
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,

    /// GitBucket host or base URL
    #[arg(long, short = 'H', global = true, env = "GB_HOST")]
    pub hostname: Option<String>,

    /// Repository in OWNER/REPO format
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
    /// Work with issues
    Issue(issue::IssueArgs),
    /// Work with labels
    Label(label::LabelArgs),
    /// Work with pull requests
    Pr(pr::PrArgs),
    /// Open the repository in a web browser
    Browse,
}
