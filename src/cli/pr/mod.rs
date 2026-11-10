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
pub struct PrArgs {
    #[command(subcommand)]
    pub command: PrCommand,
}

#[derive(Subcommand)]
pub enum PrCommand {
    /// List pull requests
    #[command(
        after_help = "Fetches all PR pages and sorts by number descending. JSON verifies each PR with canonical detail, including fork ownership. Incomplete traversal or detail verification fails without a partial list. GB_PAGINATION_MAX_PAGES sets the traversal safety cap (default 1000)."
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
        after_help = "JSON preserves API fields. --comments defaults to ordinary discussion. --comments --kind all adds review_comments and comment_coverage from the verified GitBucket 4.44 Web conversation. Public/token-readable pages need no password; only 401/403 starts Web sign-in. Visible reviews do not prove complete stored history or valid Git positions."
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
    #[command(
        after_help = "Title, body and state always use the PR REST API. --web permits only assignee changes; it does not force REST-supported fields through the Web UI."
    )]
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
    Merge {
        /// PR number
        number: u64,
        /// Merge commit message
        #[arg(long, short)]
        message: Option<String>,
    },
    /// Checkout a pull request branch locally
    #[command(
        after_help = "Checks out the saved PR head SHA, including for closed/merged PRs. Requires complete saved SHAs and a clean working tree/index, including untracked files. Creates pr-N or reuses it only for the recorded target at the same commit. Existing branches are never reset; rename legacy/conflicting branches yourself before retrying. Submodule changes must be committed or moved; submodules are not automatically fetched or checked out.\n\nHTTP(S) fetches reuse the configured user/token only within the selected GitBucket origin and base path. Authenticated transfers do not follow redirects; configure the direct clone URL."
    )]
    Checkout {
        /// PR number
        number: u64,
    },
    /// View the diff of a pull request
    #[command(
        after_help = "Requires a local Git repository. Compares the exact saved base/head SHAs, including for closed/merged PRs; current branch tips and text diff URLs cannot substitute for unavailable saved objects. A verified empty diff succeeds. Terminal output uses PAGER unless --no-pager is passed. Cancellation stops the pager and returns a cancelled error.\n\nHTTP(S) fetches reuse the configured user/token only within the selected GitBucket origin and base path. Authenticated transfers do not follow redirects; configure the direct clone URL."
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
    after_help = "Ordinary discussion writes use REST. --file/--line/--side select an inline Web write and require --web and a local Git repository. Rename paths use the destination; deleted files use the old path. Only UTF-8 text diff hunks with 3 context lines are supported. Create/reply recheck that the PR is open and unmerged immediately before POST. The saved head is checked before/after, but the server has no atomic head guard.\n\nCreate/reply intents are reserved under GB_CONFIG_DIR/review-writes before POST. The same intent is only reconciled on retry; an uncertain result is never automatically resent. Keep this directory and use a stable --request-id for automation. A new key deliberately permits another identical submission. See docs/REVIEW_WRITES.md."
)]
pub struct PrCommentArgs {
    #[command(subcommand)]
    pub command: Option<PrCommentCommand>,
    /// PR number
    pub number: Option<u64>,
    /// Comment body (prompts when omitted)
    #[arg(long, short)]
    pub body: Option<String>,
    /// Read UTF-8 body from a file
    #[arg(long, conflicts_with = "body")]
    pub body_file: Option<std::path::PathBuf>,
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
    /// Edit your own visible current inline comment (also changes other views of that record)
    Edit {
        #[command(flatten)]
        target: ReviewTargetArgs,
        #[arg(long, short, conflicts_with = "body_file")]
        body: Option<String>,
        #[arg(long)]
        body_file: Option<std::path::PathBuf>,
    },
    /// Add at the selected current position group; no persistent parent-ID relation
    Reply {
        #[command(flatten)]
        target: ReviewTargetArgs,
        #[arg(long, short, conflicts_with = "body_file")]
        body: Option<String>,
        #[arg(long)]
        body_file: Option<std::path::PathBuf>,
        #[arg(long)]
        request_id: Option<String>,
    },
    /// Delete your own visible current inline comment (also removes it from other views)
    Delete(ReviewTargetArgs),
    /// List comments on a pull request
    #[command(
        after_help = "discussion preserves the legacy JSON array. review/all return comments, review_comments and comment_coverage. Review HTML follows the verified GitBucket 4.44 layout; public reads need no Web password, while private reads may need GB_PASSWORD. Historical completeness, raw Markdown and original positions can be unknown."
    )]
    List {
        /// PR number
        number: u64,
        /// Comment sources: discussion (default), review, all; review uses Web HTML with sign-in only if needed
        #[arg(long, default_value = "discussion")]
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
                    body,
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
                    body,
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
        PrCommand::Merge { number, message } => {
            write::merge(cli_hostname, cli_repo, cli_profile, number, message).await
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
                        None,
                    ),
                )
                .await
            }
            Some(PrCommentCommand::Reply {
                target,
                body,
                body_file,
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
                let body = review_write::body_file(args.body, args.body_file)?;
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
