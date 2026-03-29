mod api;
mod cli;
mod config;
mod error;
mod models;
mod output;

use clap::Parser;
use cli::{Cli, Commands};

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    let result = match cli.command {
        Commands::Auth(args) => cli::auth::run(args, &cli.hostname).await,
        Commands::Repo(args) => cli::repo::run(args, &cli.hostname, &cli.repo).await,
        Commands::Config(args) => cli::config::run(args).await,
        Commands::Issue(args) => cli::issue::run(args, &cli.hostname, &cli.repo).await,
        Commands::Pr(args) => cli::pr::run(args, &cli.hostname, &cli.repo).await,
        Commands::Browse => browse(&cli.hostname, &cli.repo).await,
    };

    if let Err(e) = result {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}

async fn browse(hostname: &Option<String>, cli_repo: &Option<String>) -> error::Result<()> {
    let hostname = cli::common::resolve_hostname(hostname)?;
    let (owner, repo) = cli::common::resolve_repo(cli_repo)?;
    let client = cli::common::create_client(&hostname)?;
    let url = client.web_url(&format!("/{}/{}", owner, repo));
    open::that(&url)
        .map_err(|e| error::GbError::Other(format!("Failed to open browser: {}", e)))?;
    println!("Opening {} in your browser.", url);
    Ok(())
}
