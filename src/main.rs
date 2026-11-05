#[macro_export]
macro_rules! eprintln {
    () => {
        $crate::output::stderr_line(format_args!(""))
    };
    ($($arg:tt)*) => {
        $crate::output::stderr_line(format_args!($($arg)*))
    };
}

#[macro_export]
macro_rules! eprint {
    ($($arg:tt)*) => {
        $crate::output::stderr_write(format_args!($($arg)*))
    };
}

mod api;
mod cancel;
mod cli;
mod config;
mod error;
mod git;
mod limits;
mod models;
mod operation;
mod output;
mod target;

#[cfg(test)]
mod test_support;

use clap::Parser;
use cli::{Cli, Commands};
use serde::Serialize;

fn main() {
    if git::run_credential_helper() {
        return;
    }
    run_cli();
}

#[tokio::main]
async fn run_cli() {
    let cli = Cli::parse();
    output::set_suppress_stderr(cli.json_errors);

    if let Err(err) = cancel::install() {
        let err = error::GbError::Io(err);
        if cli.json_errors {
            print_json_error(&err);
        } else {
            std::eprintln!("Error: {err}");
        }
        std::process::exit(1);
    }

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
        if cli.json_errors {
            print_json_error(&e);
        } else {
            std::eprintln!("Error: {}", e);
        }
        std::process::exit(1);
    }
}

#[derive(Serialize)]
struct ErrorOutput {
    error: ErrorBody,
}

#[derive(Serialize)]
struct ErrorBody {
    #[serde(skip_serializing_if = "Option::is_none")]
    outcome: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    operations: Option<Vec<operation::Operation>>,
    code: &'static str,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    cause: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    status: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    exit_code: Option<i32>,
}

fn print_json_error(error: &error::GbError) {
    let output = ErrorOutput {
        error: ErrorBody {
            outcome: error.outcome(),
            operations: error.operations().map(<[_]>::to_vec),
            code: error.code(),
            message: error.to_string(),
            cause: error.cause_code(),
            status: error.status(),
            exit_code: Some(1),
        },
    };
    match serde_json::to_string(&output) {
        Ok(json) => std::eprintln!("{json}"),
        Err(_) => std::eprintln!("Error: {}", error),
    }
}

async fn browse(
    hostname: &Option<String>,
    cli_repo: &Option<String>,
    cli_profile: &Option<String>,
) -> error::Result<()> {
    let ctx = cli::common::RepoContext::resolve(hostname, cli_repo, cli_profile)?;
    let url = ctx.client.web_url(&format!("/{}/{}", ctx.owner, ctx.repo));
    output::open_web_url(&url)
}
