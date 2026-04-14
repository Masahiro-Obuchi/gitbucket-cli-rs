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
    },
    /// Create a pull request
    Create {
        /// PR title (prompts when omitted)
        #[arg(long, short)]
        title: Option<String>,
        /// PR body (prompts when omitted)
        #[arg(long, short)]
        body: Option<String>,
        /// Head branch (defaults to current branch)
        #[arg(long)]
        head: Option<String>,
        /// Base branch (prompts with main as the default when omitted)
        #[arg(long, short = 'B')]
        base: Option<String>,
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
        } => read::view(cli_hostname, cli_repo, number, comments, web).await,
        PrCommand::Create {
            title,
            body,
            head,
            base,
        } => write::create(cli_hostname, cli_repo, title, body, head, base).await,
        PrCommand::Close { number } => write::close(cli_hostname, cli_repo, number).await,
        PrCommand::Merge { number, message } => {
            write::merge(cli_hostname, cli_repo, number, message).await
        }
        PrCommand::Checkout { number } => worktree::checkout(cli_hostname, cli_repo, number).await,
        PrCommand::Diff { number } => worktree::diff(cli_hostname, cli_repo, number).await,
        PrCommand::Comment { number, body } => {
            write::comment(cli_hostname, cli_repo, number, body).await
        }
    }
}
