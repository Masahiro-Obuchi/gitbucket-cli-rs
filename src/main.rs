#[macro_export]
macro_rules! println {
    () => { $crate::output::stdout_line(format_args!("")) };
    ($($arg:tt)*) => { $crate::output::stdout_line(format_args!($($arg)*)) };
}

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => { $crate::output::stdout_write(format_args!($($arg)*)) };
}

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
mod application;
mod cancel;
mod cli;
mod config;
mod domain;
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

fn main() {
    if git::run_credential_helper() {
        return;
    }
    run_cli();
}

#[tokio::main]
async fn run_cli() {
    let args: Vec<_> = std::env::args_os().collect();
    let requested_json = cli::parse::json_errors_requested(&args);
    let cli = match Cli::try_parse_from(&args) {
        Ok(cli) => cli,
        Err(error) => {
            if error.exit_code() == 0 {
                if let Err(error) = error.print() {
                    if error.kind() != std::io::ErrorKind::BrokenPipe {
                        output::report_error(&error::GbError::Io(error), requested_json, None);
                        std::process::exit(1);
                    }
                }
                return;
            }
            if requested_json {
                output::argument_error(&error.kind().to_string(), cli::parse::cause(error.kind()));
            } else {
                let _ = error.print();
            }
            std::process::exit(2);
        }
    };
    let mut token = match &cli.command {
        Commands::Auth(cli::auth::AuthArgs {
            command: cli::auth::AuthCommand::Login { token, .. },
            ..
        }) => token.clone(),
        _ => None,
    };
    output::set_suppress_stderr(cli.json_errors);

    if let Err(err) = cancel::install() {
        let err = error::GbError::Io(err);
        output::report_error(&err, cli.json_errors, token.as_deref());
        std::process::exit(1);
    }

    let result = match cli.command {
        Commands::Api(args) => cli::api::run(args, &cli.hostname, &cli.profile).await,
        Commands::Auth(args) => cli::auth::run(args, &cli.hostname, &cli.profile, &mut token).await,
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

    if let Err(e) = output::finish(result) {
        output::report_error(&e, cli.json_errors, token.as_deref());
        std::process::exit(1);
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
