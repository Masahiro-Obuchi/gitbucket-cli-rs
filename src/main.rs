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
        Commands::Api(args) => cli::api::run(args, &cli.hostname, &cli.profile).await,
        Commands::Auth(args) => cli::auth::run(args, &cli.hostname, &cli.profile).await,
        Commands::Repo(args) => cli::repo::run(args, &cli.hostname, &cli.repo, &cli.profile).await,
        Commands::Config(args) => cli::config::run(args, &cli.profile).await,
        Commands::Completion(args) => cli::completion::run(args).await,
        Commands::Issue(args) => {
            cli::issue::run(args, &cli.hostname, &cli.repo, &cli.profile).await
        }
        Commands::Label(args) => {
            cli::label::run(args, &cli.hostname, &cli.repo, &cli.profile).await
        }
        Commands::Milestone(args) => {
            cli::milestone::run(args, &cli.hostname, &cli.repo, &cli.profile).await
        }
        Commands::Pr(args) => cli::pr::run(args, &cli.hostname, &cli.repo, &cli.profile).await,
        Commands::Browse => browse(&cli.hostname, &cli.repo, &cli.profile).await,
    };

    if let Err(e) = result {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}

async fn browse(
    hostname: &Option<String>,
    cli_repo: &Option<String>,
    cli_profile: &Option<String>,
) -> error::Result<()> {
    let hostname = cli::common::resolve_hostname(hostname, cli_profile)?;
    let (owner, repo) = cli::common::resolve_repo(cli_repo, cli_profile)?;
    let client = cli::common::create_client(&hostname, cli_profile)?;
    let url = client.web_url(&format!("/{}/{}", owner, repo));
    open::that(&url)
        .map_err(|e| error::GbError::Other(format!("Failed to open browser: {}", e)))?;
    println!("Opening {} in your browser.", url);
    Ok(())
}
