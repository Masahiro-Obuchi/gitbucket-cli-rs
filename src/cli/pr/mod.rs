use clap::{Args, Subcommand};

use crate::cli::common::normalize_str_vec;

mod git;
mod read;
mod worktree;
mod write;

#[derive(Args)]
pub struct PrArgs {
    #[command(subcommand)]
    pub command: PrCommand,
}

#[derive(Subcommand)]
pub enum PrCommand {
    /// List pull requests
    List {
        /// Filter by state (open, closed, all)
        #[arg(
            long,
            short,
            default_value = "open",
            value_parser = ["open", "closed", "all"],
            ignore_case = true
        )]
        state: String,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
    /// View a pull request
    View {
        /// PR number
        number: u64,
        /// Show comments
        #[arg(long, short)]
        comments: bool,
        /// Open in browser
        #[arg(long, short)]
        web: bool,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
    /// Create a pull request
    Create {
        /// PR title (prompts when omitted)
        #[arg(long, short)]
        title: Option<String>,
        /// PR body (prompts when omitted)
        #[arg(long, short)]
        body: Option<String>,
        /// Head branch (defaults to current branch). For cross-repo PRs, use OWNER:BRANCH or pass --head-owner.
        #[arg(long)]
        head: Option<String>,
        /// Owner for the head branch; sends the head as OWNER:BRANCH
        #[arg(long = "head-owner")]
        head_owner: Option<String>,
        /// Base branch (prompts with main as the default when omitted)
        #[arg(long, short = 'B')]
        base: Option<String>,
        /// Output the created pull request as JSON
        #[arg(long)]
        json: bool,
        /// Return an existing open PR for the same head/base instead of creating a duplicate
        #[arg(long)]
        detect_existing: bool,
    },
    /// Edit a pull request
    Edit {
        /// PR number
        number: u64,
        /// New PR title
        #[arg(long, short)]
        title: Option<String>,
        /// New PR body
        #[arg(long, short)]
        body: Option<String>,
        /// Add assignee username (repeatable or comma-separated)
        #[arg(long = "add-assignee", value_delimiter = ',')]
        add_assignee: Vec<String>,
        /// Remove assignee username (repeatable or comma-separated)
        #[arg(long = "remove-assignee", value_delimiter = ',')]
        remove_assignee: Vec<String>,
        /// Update PR state (open or closed)
        #[arg(long, value_parser = ["open", "closed"], ignore_case = true)]
        state: Option<String>,
        /// Allow GitBucket web UI fallback when REST PR edit is unavailable
        #[arg(long)]
        web: bool,
    },
    /// Close a pull request
    Close {
        /// PR number
        number: u64,
    },
    /// Merge a pull request
    Merge {
        /// PR number
        number: u64,
        /// Merge commit message
        #[arg(long, short)]
        message: Option<String>,
    },
    /// Checkout a pull request branch locally
    Checkout {
        /// PR number
        number: u64,
    },
    /// View the diff of a pull request
    Diff {
        /// PR number
        number: u64,
        /// Do not use a pager
        #[arg(long)]
        no_pager: bool,
    },
    /// Add, edit, or list comments on a pull request
    Comment(PrCommentArgs),
}

#[derive(Args)]
#[command(args_conflicts_with_subcommands = true, subcommand_negates_reqs = true)]
pub struct PrCommentArgs {
    #[command(subcommand)]
    pub command: Option<PrCommentCommand>,
    /// PR number
    pub number: Option<u64>,
    /// Comment body (prompts when omitted)
    #[arg(long, short)]
    pub body: Option<String>,
    /// Edit your last comment instead of adding a new one
    #[arg(long)]
    pub edit_last: bool,
    /// Output the comment as JSON
    #[arg(long)]
    pub json: bool,
}

#[derive(Subcommand)]
pub enum PrCommentCommand {
    /// List comments on a pull request
    List {
        /// PR number
        number: u64,
        /// Output comments as JSON
        #[arg(long)]
        json: bool,
    },
}

pub async fn run(
    args: PrArgs,
    cli_hostname: &Option<String>,
    cli_repo: &Option<String>,
    cli_profile: &Option<String>,
) -> crate::error::Result<()> {
    match args.command {
        PrCommand::List { state, json } => {
            read::list(cli_hostname, cli_repo, cli_profile, &state, json).await
        }
        PrCommand::View {
            number,
            comments,
            web,
            json,
        } => {
            read::view(
                cli_hostname,
                cli_repo,
                cli_profile,
                number,
                comments,
                web,
                json,
            )
            .await
        }
        PrCommand::Create {
            title,
            body,
            head,
            head_owner,
            base,
            json,
            detect_existing,
        } => {
            write::create(
                cli_hostname,
                cli_repo,
                cli_profile,
                title,
                body,
                head,
                head_owner,
                base,
                json,
                detect_existing,
            )
            .await
        }
        PrCommand::Edit {
            number,
            title,
            body,
            add_assignee,
            remove_assignee,
            state,
            web,
        } => {
            write::edit(
                cli_hostname,
                cli_repo,
                cli_profile,
                number,
                title,
                body,
                normalize_str_vec(add_assignee),
                normalize_str_vec(remove_assignee),
                state,
                web,
            )
            .await
        }
        PrCommand::Close { number } => {
            write::close(cli_hostname, cli_repo, cli_profile, number).await
        }
        PrCommand::Merge { number, message } => {
            write::merge(cli_hostname, cli_repo, cli_profile, number, message).await
        }
        PrCommand::Checkout { number } => {
            worktree::checkout(cli_hostname, cli_repo, cli_profile, number).await
        }
        PrCommand::Diff { number, no_pager } => {
            worktree::diff(cli_hostname, cli_repo, cli_profile, number, no_pager).await
        }
        PrCommand::Comment(args) => match args.command {
            Some(PrCommentCommand::List { number, json }) => {
                read::list_comments(cli_hostname, cli_repo, cli_profile, number, json).await
            }
            None => {
                let number = args.number.ok_or_else(|| {
                    crate::error::GbError::Other(
                        "PR number is required. Use `gb pr comment <NUMBER>` or `gb pr comment list <NUMBER>`.".into(),
                    )
                })?;
                write::comment(
                    cli_hostname,
                    cli_repo,
                    cli_profile,
                    number,
                    args.body,
                    args.edit_last,
                    args.json,
                )
                .await
            }
        },
    }
}
