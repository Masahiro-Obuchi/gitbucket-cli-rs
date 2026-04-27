use clap::{Args, Subcommand};

use crate::cli::common::normalize_str_vec;

mod read;
mod write;

#[derive(Args)]
pub struct IssueArgs {
    #[command(subcommand)]
    pub command: IssueCommand,
}

#[derive(Subcommand)]
pub enum IssueCommand {
    /// List issues
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
        /// Do not use a pager
        #[arg(long)]
        no_pager: bool,
    },
    /// View an issue (use --comments to include comments)
    View {
        /// Issue number
        number: u64,
        /// Include comments in the output
        #[arg(long, short)]
        comments: bool,
        /// Open in browser
        #[arg(long, short)]
        web: bool,
        /// Print raw JSON response
        #[arg(long)]
        json: bool,
        /// Do not use a pager
        #[arg(long)]
        no_pager: bool,
    },
    /// Create a new issue
    Create {
        /// Issue title (prompts when omitted)
        #[arg(long, short)]
        title: Option<String>,
        /// Issue body (prompts when omitted)
        #[arg(long, short)]
        body: Option<String>,
        /// Label name (repeatable or comma-separated)
        #[arg(long, short, value_delimiter = ',')]
        label: Vec<String>,
        /// Assignee username (repeatable or comma-separated)
        #[arg(long, short, value_delimiter = ',')]
        assignee: Vec<String>,
    },
    /// Edit an issue
    Edit {
        /// Issue number
        number: u64,
        /// New issue title
        #[arg(long, short)]
        title: Option<String>,
        /// New issue body
        #[arg(long, short)]
        body: Option<String>,
        /// Add label name (repeatable or comma-separated)
        #[arg(long = "add-label", value_delimiter = ',')]
        add_label: Vec<String>,
        /// Remove label name (repeatable or comma-separated)
        #[arg(long = "remove-label", value_delimiter = ',')]
        remove_label: Vec<String>,
        /// Add assignee username (repeatable or comma-separated)
        #[arg(long = "add-assignee", value_delimiter = ',')]
        add_assignee: Vec<String>,
        /// Remove assignee username (repeatable or comma-separated)
        #[arg(long = "remove-assignee", value_delimiter = ',')]
        remove_assignee: Vec<String>,
        /// Set milestone number
        #[arg(long)]
        milestone: Option<u64>,
        /// Remove the current milestone
        #[arg(long)]
        remove_milestone: bool,
        /// Update issue state (open or closed)
        #[arg(long, value_parser = ["open", "closed"], ignore_case = true)]
        state: Option<String>,
    },
    /// Close an issue
    Close {
        /// Issue number
        number: u64,
    },
    /// Reopen an issue
    Reopen {
        /// Issue number
        number: u64,
    },
    /// Add or edit a comment on an issue
    Comment {
        /// Issue number
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
    args: IssueArgs,
    cli_hostname: &Option<String>,
    cli_repo: &Option<String>,
    cli_profile: &Option<String>,
) -> crate::error::Result<()> {
    match args.command {
        IssueCommand::List {
            state,
            json,
            no_pager,
        } => read::list(cli_hostname, cli_repo, cli_profile, &state, json, no_pager).await,
        IssueCommand::View {
            number,
            comments,
            web,
            json,
            no_pager,
        } => {
            read::view(
                cli_hostname,
                cli_repo,
                cli_profile,
                read::ViewOptions {
                    number,
                    show_comments: comments,
                    web,
                    json,
                    no_pager,
                },
            )
            .await
        }
        IssueCommand::Create {
            title,
            body,
            label,
            assignee,
        } => {
            write::create(
                cli_hostname,
                cli_repo,
                cli_profile,
                title,
                body,
                normalize_str_vec(label),
                normalize_str_vec(assignee),
            )
            .await
        }
        IssueCommand::Edit {
            number,
            title,
            body,
            add_label,
            remove_label,
            add_assignee,
            remove_assignee,
            milestone,
            remove_milestone,
            state,
        } => {
            write::edit(
                cli_hostname,
                cli_repo,
                cli_profile,
                number,
                title,
                body,
                normalize_str_vec(add_label),
                normalize_str_vec(remove_label),
                normalize_str_vec(add_assignee),
                normalize_str_vec(remove_assignee),
                milestone,
                remove_milestone,
                state,
            )
            .await
        }
        IssueCommand::Close { number } => {
            write::close(cli_hostname, cli_repo, cli_profile, number).await
        }
        IssueCommand::Reopen { number } => {
            write::reopen(cli_hostname, cli_repo, cli_profile, number).await
        }
        IssueCommand::Comment {
            number,
            body,
            edit_last,
        } => write::comment(cli_hostname, cli_repo, cli_profile, number, body, edit_last).await,
    }
}
