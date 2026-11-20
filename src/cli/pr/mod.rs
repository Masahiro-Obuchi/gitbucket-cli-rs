use clap::{Args, Subcommand};

use crate::cli::common::normalize_str_vec;
use crate::domain::review_comment::CommentSelection;

mod checkout;
mod git;
mod read;
mod review_write;
mod target;
mod worktree;
mod write;

#[derive(Args)]
#[command(after_long_help = "See docs/usage/pull-requests.md for supported PR workflows.")]
pub struct PrArgs {
    #[command(subcommand)]
    pub command: PrCommand,
}

#[derive(Subcommand)]
pub enum PrCommand {
    /// List pull requests
    #[command(
        after_long_help = "JSON verifies canonical details for every PR. See docs/reference/automation.md."
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
        /// Output as JSON
        #[arg(long)]
        json: bool,
        /// Do not use a pager
        #[arg(long)]
        no_pager: bool,
    },
    /// View a pull request
    #[command(
        after_long_help = "With --comments, the default is --kind all. JSON adds comment_coverage."
    )]
    View {
        /// PR number
        number: u64,
        /// Show comments
        #[arg(long, short)]
        comments: bool,
        /// Comment sources: discussion (default), review, all; review uses Web HTML with sign-in only if needed
        #[arg(long = "kind", requires = "comments")]
        kind: Option<CommentSelection>,
        /// Open in browser
        #[arg(long, short)]
        web: bool,
        /// Output as JSON
        #[arg(long)]
        json: bool,
        /// Do not use a pager
        #[arg(long)]
        no_pager: bool,
    },
    /// Create a pull request
    #[command(
        after_long_help = "Omitted --base uses the target default branch. An inferred head must match the published commit. See docs/usage/pull-requests.md."
    )]
    Create {
        /// PR title (prompts when omitted)
        #[arg(long, short)]
        title: Option<String>,
        /// PR body (optional without a terminal)
        #[arg(long, short)]
        body: Option<String>,
        /// Read UTF-8 body from a file, or - for stdin
        #[arg(long, conflicts_with = "body", value_name = "FILE|-")]
        body_file: Option<std::path::PathBuf>,
        /// Compose body in GB_EDITOR, VISUAL, EDITOR or the default editor (terminal only)
        #[arg(long, short = 'e', conflicts_with_all = ["body", "body_file"])]
        editor: bool,
        /// Published head branch (default: verify and use current branch). Cross-repo: OWNER:BRANCH or --head-owner.
        #[arg(long)]
        head: Option<String>,
        /// Owner for the head branch; sends the head as OWNER:BRANCH
        #[arg(long = "head-owner")]
        head_owner: Option<String>,
        /// Base branch (defaults to the target repository default branch)
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
    #[command(after_long_help = "--web permits assignee changes. Title, body and state use REST.")]
    Edit {
        /// PR number
        number: u64,
        /// New PR title
        #[arg(long, short)]
        title: Option<String>,
        /// New PR body
        #[arg(long, short)]
        body: Option<String>,
        /// Read UTF-8 body from a file, or - for stdin
        #[arg(long, conflicts_with = "body", value_name = "FILE|-")]
        body_file: Option<std::path::PathBuf>,
        /// Compose body in GB_EDITOR, VISUAL, EDITOR or the default editor (terminal only)
        #[arg(long, short = 'e', conflicts_with_all = ["body", "body_file"])]
        editor: bool,
        /// Add assignee username (repeatable or comma-separated)
        #[arg(long = "add-assignee", value_delimiter = ',')]
        add_assignee: Vec<String>,
        /// Remove assignee username (repeatable or comma-separated)
        #[arg(long = "remove-assignee", value_delimiter = ',')]
        remove_assignee: Vec<String>,
        /// Update PR state (open or closed)
        #[arg(long, value_parser = ["open", "closed"], ignore_case = true)]
        state: Option<String>,
        /// Permit GitBucket Web UI assignee changes (title/body/state use REST)
        #[arg(long)]
        web: bool,
    },
    /// Close a pull request
    Close {
        /// PR number
        number: u64,
    },
    /// Reopen a closed, unmerged pull request
    Reopen {
        /// PR number
        number: u64,
    },
    /// Merge a pull request
    #[command(
        after_long_help = "Head checks are client-side and non-atomic. Use --match-head-commit to pin the reviewed head. See docs/reference/merge.md."
    )]
    Merge {
        /// PR number
        number: u64,
        /// Merge commit message
        #[arg(long, short)]
        message: Option<String>,
        /// Full reviewed head SHA (client checks only; not an atomic server guard)
        #[arg(long, value_parser = crate::domain::pull_request::CommitId::parse)]
        match_head_commit: Option<crate::domain::pull_request::CommitId>,
        /// Print the result and explicit client-check/atomic=false metadata as JSON
        #[arg(long)]
        json: bool,
    },
    /// Checkout a pull request branch locally
    #[command(
        after_long_help = "Requires a clean worktree and checks out the saved PR head. See docs/reference/pull-request-snapshots.md."
    )]
    Checkout {
        /// PR number
        number: u64,
    },
    /// View the diff of a pull request
    #[command(
        after_long_help = "Requires a local Git repository. Compares the saved base and head. See docs/reference/pull-request-snapshots.md."
    )]
    Diff {
        /// PR number
        number: u64,
        /// Do not use a pager
        #[arg(long)]
        no_pager: bool,
    },
    /// Add, edit, reply to, delete, or list comments on a pull request
    Comment(PrCommentArgs),
}

#[derive(Args)]
#[command(
    args_conflicts_with_subcommands = true,
    subcommand_negates_reqs = true,
    after_long_help = "Inline writes require --web and local Git. For an uncertain create or reply, keep the original retry receipt, actor and --request-id. See docs/reference/review-writes.md."
)]
pub struct PrCommentArgs {
    #[command(subcommand)]
    pub command: Option<PrCommentCommand>,
    /// PR number
    pub number: Option<u64>,
    /// Comment body (prompts when omitted)
    #[arg(long, short)]
    pub body: Option<String>,
    /// Read UTF-8 body from a file, or - for stdin
    #[arg(long, conflicts_with = "body")]
    pub body_file: Option<std::path::PathBuf>,
    /// Compose body in GB_EDITOR, VISUAL, EDITOR or the default editor (terminal only)
    #[arg(long, short = 'e', conflicts_with_all = ["body", "body_file"])]
    pub editor: bool,
    /// Exact path in the saved PR diff (destination path for renames)
    #[arg(long, requires_all = ["line", "side", "web"], conflicts_with = "edit_last")]
    pub file: Option<String>,
    /// Old/new file line number, not a patch offset
    #[arg(long, requires = "file")]
    pub line: Option<u64>,
    /// Side of the saved diff (old or new)
    #[arg(long, requires = "file")]
    pub side: Option<crate::domain::review_write::Side>,
    /// Expected full saved PR head SHA; defaults to the head read at start
    #[arg(long, requires = "file")]
    pub commit: Option<String>,
    /// Explicitly permit Cookie-authenticated inline Web writes
    #[arg(long, requires = "file")]
    pub web: bool,
    /// Stable retry key; a new key permits an intentional identical submission
    #[arg(long, requires = "file")]
    pub request_id: Option<String>,
    /// Edit your last ordinary discussion comment instead of adding a new one
    #[arg(long)]
    pub edit_last: bool,
    /// Output the comment as JSON
    #[arg(long)]
    pub json: bool,
}

#[derive(Subcommand)]
pub enum PrCommentCommand {
    /// Edit your current inline comment
    Edit {
        #[command(flatten)]
        target: ReviewTargetArgs,
        #[arg(long, short, conflicts_with = "body_file")]
        body: Option<String>,
        /// Read UTF-8 body from a file, or - for stdin
        #[arg(long, conflicts_with = "body", value_name = "FILE|-")]
        body_file: Option<std::path::PathBuf>,
        /// Compose body in GB_EDITOR, VISUAL, EDITOR or the default editor (terminal only)
        #[arg(long, short = 'e', conflicts_with_all = ["body", "body_file"])]
        editor: bool,
    },
    /// Reply in an inline position group
    Reply {
        #[command(flatten)]
        target: ReviewTargetArgs,
        #[arg(long, short, conflicts_with = "body_file")]
        body: Option<String>,
        /// Read UTF-8 body from a file, or - for stdin
        #[arg(long, conflicts_with = "body", value_name = "FILE|-")]
        body_file: Option<std::path::PathBuf>,
        /// Compose body in GB_EDITOR, VISUAL, EDITOR or the default editor (terminal only)
        #[arg(long, short = 'e', conflicts_with_all = ["body", "body_file"])]
        editor: bool,
        #[arg(long)]
        request_id: Option<String>,
    },
    /// Delete your current inline comment
    Delete(ReviewTargetArgs),
    /// List comments on a pull request
    #[command(
        after_long_help = "The default shows discussion and visible reviews. JSON reports coverage for both sources. Private reviews may need GB_PASSWORD."
    )]
    List {
        /// PR number
        number: u64,
        /// Comment sources: discussion (default), review, all; review uses Web HTML with sign-in only if needed
        #[arg(long, default_value = "all")]
        kind: CommentSelection,
        /// Output comments as JSON
        #[arg(long)]
        json: bool,
        /// Do not use a pager
        #[arg(long)]
        no_pager: bool,
    },
}

#[derive(Args)]
pub struct ReviewTargetArgs {
    pub number: u64,
    /// Storage kind; ordinary discussion IDs are never accepted here
    #[arg(long, required = true, value_parser = ["inline"])]
    pub kind: String,
    #[arg(long)]
    pub id: u64,
    /// Expected full saved PR head SHA
    #[arg(long)]
    pub commit: Option<String>,
    /// Permit Cookie-authenticated Web changes to this CommitComment record
    #[arg(long, required = true)]
    pub web: bool,
    #[arg(long)]
    pub json: bool,
}

pub async fn run(
    args: PrArgs,
    cli_hostname: &Option<String>,
    cli_repo: &Option<String>,
    cli_profile: &Option<String>,
) -> crate::error::Result<()> {
    match args.command {
        PrCommand::List {
            state,
            json,
            no_pager,
        } => read::list(cli_hostname, cli_repo, cli_profile, &state, json, no_pager).await,
        PrCommand::View {
            number,
            comments,
            kind,
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
                    selection: kind.unwrap_or_default(),
                    web,
                    json,
                    no_pager,
                },
            )
            .await
        }
        PrCommand::Create {
            title,
            body,
            body_file,
            editor,
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
                write::CreateRequest {
                    title,
                    body: super::input::body(body, body_file, editor).await?,
                    head,
                    head_owner,
                    base,
                    json,
                    detect_existing,
                },
            )
            .await
        }
        PrCommand::Edit {
            number,
            title,
            body,
            body_file,
            editor,
            add_assignee,
            remove_assignee,
            state,
            web,
        } => {
            write::edit(
                cli_hostname,
                cli_repo,
                cli_profile,
                write::EditRequest {
                    number,
                    title,
                    body: super::input::body(body, body_file, editor).await?,
                    add_assignees: normalize_str_vec(add_assignee),
                    remove_assignees: normalize_str_vec(remove_assignee),
                    state,
                    web,
                },
            )
            .await
        }
        PrCommand::Close { number } => {
            write::close(cli_hostname, cli_repo, cli_profile, number).await
        }
        PrCommand::Reopen { number } => {
            write::reopen(cli_hostname, cli_repo, cli_profile, number).await
        }
        PrCommand::Merge {
            number,
            message,
            match_head_commit,
            json,
        } => {
            write::merge(
                cli_hostname,
                cli_repo,
                cli_profile,
                number,
                message,
                match_head_commit,
                json,
            )
            .await
        }
        PrCommand::Checkout { number } => {
            checkout::checkout(cli_hostname, cli_repo, cli_profile, number).await
        }
        PrCommand::Diff { number, no_pager } => {
            worktree::diff(cli_hostname, cli_repo, cli_profile, number, no_pager).await
        }
        PrCommand::Comment(args) => match args.command {
            Some(PrCommentCommand::Edit {
                target,
                body,
                body_file,
                editor,
            }) => {
                review_write::run(
                    cli_hostname,
                    cli_repo,
                    cli_profile,
                    review_write::Request::identified(
                        target,
                        review_write::Action::Edit,
                        body,
                        body_file,
                        editor,
                        None,
                    ),
                )
                .await
            }
            Some(PrCommentCommand::Reply {
                target,
                body,
                body_file,
                editor,
                request_id,
            }) => {
                review_write::run(
                    cli_hostname,
                    cli_repo,
                    cli_profile,
                    review_write::Request::identified(
                        target,
                        review_write::Action::Reply,
                        body,
                        body_file,
                        editor,
                        request_id,
                    ),
                )
                .await
            }
            Some(PrCommentCommand::Delete(target)) => {
                review_write::run(
                    cli_hostname,
                    cli_repo,
                    cli_profile,
                    review_write::Request::identified(
                        target,
                        review_write::Action::Delete,
                        None,
                        None,
                        false,
                        None,
                    ),
                )
                .await
            }
            Some(PrCommentCommand::List {
                number,
                kind,
                json,
                no_pager,
            }) => {
                read::list_comments(
                    cli_hostname,
                    cli_repo,
                    cli_profile,
                    number,
                    kind,
                    json,
                    no_pager,
                )
                .await
            }
            None => {
                let number = args.number.ok_or_else(|| {
                    crate::error::GbError::Other(
                        "PR number is required. Use `gb pr comment <NUMBER>` or `gb pr comment list <NUMBER>`.".into(),
                    )
                })?;
                if args.file.is_some() {
                    return review_write::run(
                        cli_hostname,
                        cli_repo,
                        cli_profile,
                        review_write::Request::create(number, args)?,
                    )
                    .await;
                }
                let body = super::input::body(args.body, args.body_file, args.editor).await?;
                write::comment(
                    cli_hostname,
                    cli_repo,
                    cli_profile,
                    number,
                    body,
                    args.edit_last,
                    args.json,
                )
                .await
            }
        },
    }
}
