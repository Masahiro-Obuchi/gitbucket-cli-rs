use clap::{Args, Subcommand};

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
    },
    /// Add a comment to a pull request
    Comment {
        /// PR number
        number: u64,
        /// Comment body (prompts when omitted)
        #[arg(long, short)]
        body: Option<String>,
        /// Edit your last comment instead of adding a new one
        #[arg(long)]
        edit_last: bool,
    },
}

pub async fn run(
    args: PrArgs,
    cli_hostname: &Option<String>,
    cli_repo: &Option<String>,
) -> crate::error::Result<()> {
    match args.command {
        PrCommand::List { state, json } => read::list(cli_hostname, cli_repo, &state, json).await,
        PrCommand::View {
            number,
            comments,
            web,
            json,
        } => read::view(cli_hostname, cli_repo, number, comments, web, json).await,
        PrCommand::Create {
            title,
            body,
            head,
            head_owner,
            base,
            json,
        } => {
            write::create(
                cli_hostname,
                cli_repo,
                title,
                body,
                head,
                head_owner,
                base,
                json,
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
        } => {
            write::edit(
                cli_hostname,
                cli_repo,
                number,
                title,
                body,
                normalize_str_vec(add_assignee),
                normalize_str_vec(remove_assignee),
                state,
            )
            .await
        }
        PrCommand::Close { number } => write::close(cli_hostname, cli_repo, number).await,
        PrCommand::Merge { number, message } => {
            write::merge(cli_hostname, cli_repo, number, message).await
        }
        PrCommand::Checkout { number } => worktree::checkout(cli_hostname, cli_repo, number).await,
        PrCommand::Diff { number } => worktree::diff(cli_hostname, cli_repo, number).await,
        PrCommand::Comment {
            number,
            body,
            edit_last,
        } => write::comment(cli_hostname, cli_repo, number, body, edit_last).await,
    }
}

fn normalize_str_vec(values: Vec<String>) -> Vec<String> {
    values
        .into_iter()
        .map(|v| v.trim().to_owned())
        .filter(|s| !s.is_empty())
        .collect()
}
