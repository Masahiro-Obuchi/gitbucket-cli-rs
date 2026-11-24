use clap::{Args, Subcommand};

use crate::cli::common::normalize_str_vec;

use super::json_output::JsonOutput;

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
    #[command(
        after_long_help = "Reads the complete collection. See docs/reference/automation.md."
    )]
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
        #[command(flatten)]
        json: JsonOutput,
        /// Do not use a pager
        #[arg(long)]
        no_pager: bool,
    },
    /// View an issue (use --comments to include comments)
    #[command(after_long_help = "--comments adds comment_details to the API JSON object.")]
    View {
        /// Issue number
        number: u64,
        /// Include comments in the output
        #[arg(long, short)]
        comments: bool,
        /// Open in browser
        #[arg(long, short, conflicts_with = "json_fields")]
        web: bool,
        #[command(flatten)]
        json: JsonOutput,
        /// Do not use a pager
        #[arg(long)]
        no_pager: bool,
    },
    /// Create a new issue
    #[command(
        after_long_help = "Noninteractive creation requires --title. See docs/usage/input-and-display.md."
    )]
    Create {
        /// Issue title (prompts when omitted)
        #[arg(long, short)]
        title: Option<String>,
        /// Issue body (optional without a terminal)
        #[arg(long, short)]
        body: Option<String>,
        /// Read UTF-8 body from a file, or - for stdin
        #[arg(long, conflicts_with = "body", value_name = "FILE|-")]
        body_file: Option<std::path::PathBuf>,
        /// Compose body in GB_EDITOR, VISUAL, EDITOR or the default editor (terminal only)
        #[arg(long, short = 'e', conflicts_with_all = ["body", "body_file"])]
        editor: bool,
        /// Label name (repeatable or comma-separated)
        #[arg(long, short, value_delimiter = ',')]
        label: Vec<String>,
        /// Assignee username (repeatable or comma-separated)
        #[arg(long, short, value_delimiter = ',')]
        assignee: Vec<String>,
    },
    /// Edit an issue
    #[command(
        after_long_help = "Label-only edits use REST. Other edits may need Web sign-in. See docs/usage/commands.md."
    )]
    Edit {
        /// Issue number
        number: u64,
        /// New issue title
        #[arg(long, short)]
        title: Option<String>,
        /// New issue body
        #[arg(long, short)]
        body: Option<String>,
        /// Read UTF-8 body from a file, or - for stdin
        #[arg(long, conflicts_with = "body", value_name = "FILE|-")]
        body_file: Option<std::path::PathBuf>,
        /// Compose body in GB_EDITOR, VISUAL, EDITOR or the default editor (terminal only)
        #[arg(long, short = 'e', conflicts_with_all = ["body", "body_file"])]
        editor: bool,
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
        /// Read UTF-8 body from a file, or - for stdin
        #[arg(long, conflicts_with = "body", value_name = "FILE|-")]
        body_file: Option<std::path::PathBuf>,
        /// Compose body in GB_EDITOR, VISUAL, EDITOR or the default editor (terminal only)
        #[arg(long, short = 'e', conflicts_with_all = ["body", "body_file"])]
        editor: bool,
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
            body_file,
            editor,
            label,
            assignee,
        } => {
            write::create(
                cli_hostname,
                cli_repo,
                cli_profile,
                title,
                super::input::body(body, body_file, editor).await?,
                nonempty_label_names(label),
                normalize_str_vec(assignee),
            )
            .await
        }
        IssueCommand::Edit {
            number,
            title,
            body,
            body_file,
            editor,
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
                write::EditRequest {
                    number,
                    title,
                    body: super::input::body(body, body_file, editor).await?,
                    add_labels: nonempty_label_names(add_label),
                    remove_labels: nonempty_label_names(remove_label),
                    add_assignees: normalize_str_vec(add_assignee),
                    remove_assignees: normalize_str_vec(remove_assignee),
                    milestone,
                    remove_milestone,
                    state,
                },
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
            body_file,
            editor,
            edit_last,
        } => {
            write::comment(
                cli_hostname,
                cli_repo,
                cli_profile,
                number,
                super::input::body(body, body_file, editor).await?,
                edit_last,
            )
            .await
        }
    }
}

fn nonempty_label_names(values: Vec<String>) -> Vec<String> {
    // Surrounding whitespace is part of a label's identity; trimming could
    // select a different existing label for an addition or removal.
    values
        .into_iter()
        .filter(|value| !value.is_empty())
        .collect()
}
