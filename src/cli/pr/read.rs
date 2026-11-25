use colored::Colorize;

use crate::application::pr_comments::{self, PrComments};
use crate::cli::common::{create_web_session_with_prompt, RepoContext};
use crate::cli::json_output::JsonOutput;
use crate::domain::pull_request::CommitId;
use crate::domain::review_comment::{CommentIdentity, CommentKind, CommentSelection};
use crate::error::{GbError, Result};
use crate::models::pull_request::PullRequest;
use crate::output;
use crate::output::table::format_table;
use crate::output::{format_state, page_or_print, truncate};

pub(super) struct ViewOptions {
    pub number: u64,
    pub show_comments: bool,
    pub selection: CommentSelection,
    pub web: bool,
    pub json: JsonOutput,
    pub no_pager: bool,
}

pub(super) async fn list(
    hostname: &Option<String>,
    cli_repo: &Option<String>,
    cli_profile: &Option<String>,
    state: &str,
    json: JsonOutput,
    no_pager: bool,
) -> Result<()> {
    let ctx = RepoContext::resolve(hostname, cli_repo, cli_profile)?;
    let state = crate::cli::common::normalize_list_state(state)?;

    let prs = ctx
        .client
        .list_repository_pull_requests(&ctx.owner, &ctx.repo, &state, json.enabled())
        .await?;

    if json.enabled() {
        return json.page(&prs, no_pager);
    }

    let rows: Vec<Vec<String>> = prs
        .iter()
        .map(|pr| {
            let state = if pr.merged == Some(true) {
                "merged"
            } else if pr.state == "open" && pr.draft == Some(true) {
                "draft"
            } else {
                &pr.state
            };
            let branch = pr.head.as_ref().map(|h| h.ref_name.as_str()).unwrap_or("");
            vec![
                format!("#{}", pr.number),
                format_state(state),
                truncate(&pr.title, 50),
                branch.to_string(),
                pr.user
                    .as_ref()
                    .map(|u| u.login.clone())
                    .unwrap_or_default(),
            ]
        })
        .collect();

    page_or_print(
        &format_table(&["#", "STATE", "TITLE", "BRANCH", "AUTHOR"], &rows),
        no_pager,
    )?;
    Ok(())
}

pub(super) async fn list_comments(
    hostname: &Option<String>,
    cli_repo: &Option<String>,
    cli_profile: &Option<String>,
    number: u64,
    selection: CommentSelection,
    json: JsonOutput,
    no_pager: bool,
) -> Result<()> {
    let ctx = RepoContext::resolve(hostname, cli_repo, cli_profile)?;
    let (_, head) = load_comment_target(&ctx, number, selection).await?;
    let comments = load_comments(&ctx, cli_profile, number, selection, head.as_ref()).await?;

    if json.enabled() {
        return json.page(&comments, no_pager);
    }

    page_or_print(&format_comments(&comments, selection), no_pager)?;
    Ok(())
}

async fn load_comment_target(
    ctx: &RepoContext,
    number: u64,
    selection: CommentSelection,
) -> Result<(PullRequest, Option<CommitId>)> {
    if selection.review() {
        return pr_comments::review_target(&ctx.client, &ctx.owner, &ctx.repo, number).await;
    }

    let pr = super::target::require_pull_request(ctx, number).await?;
    Ok((pr, None))
}

async fn load_comments(
    ctx: &RepoContext,
    profile: &Option<String>,
    number: u64,
    selection: CommentSelection,
    head: Option<&CommitId>,
) -> Result<PrComments> {
    let discussion = if selection.discussion() {
        Some(
            ctx.client
                .list_pr_comments(&ctx.owner, &ctx.repo, number)
                .await,
        )
    } else {
        None
    };

    if crate::cancel::requested() {
        return Err(GbError::Cancelled);
    }

    let review = if selection.review() {
        Some(
            load_reviews(
                ctx,
                profile,
                number,
                head,
                selection == CommentSelection::Review,
            )
            .await,
        )
    } else {
        None
    };

    pr_comments::combine(
        &ctx.client,
        &ctx.owner,
        &ctx.repo,
        selection,
        discussion,
        review,
    )
}

async fn load_reviews(
    ctx: &RepoContext,
    profile: &Option<String>,
    number: u64,
    head: Option<&CommitId>,
    allow_prompt: bool,
) -> Result<pr_comments::ReviewRead> {
    use crate::api::web::review::unavailable;

    let head = head.ok_or_else(|| {
        unavailable(
            "missing_head_identity",
            "A saved PR head SHA is required to verify Web comment positions.",
        )
    })?;
    let (comments, web_actor) = match ctx
        .client
        .token_review_comments(&ctx.owner, &ctx.repo, number, head)
        .await
    {
        Ok(comments) => (comments, None),
        Err(GbError::Api {
            status: 401 | 403, ..
        }) => {
            let session =
                create_web_session_with_prompt(&ctx.client, &ctx.hostname, profile, allow_prompt)
                    .await?;
            let comments = session
                .review_comments(&ctx.owner, &ctx.repo, number, head)
                .await?;
            (comments, Some(session.principal().to_owned()))
        }
        Err(error) => return Err(error),
    };
    let (_, after) = pr_comments::review_target(&ctx.client, &ctx.owner, &ctx.repo, number).await?;
    if after.as_ref() != Some(head) {
        return Err(unavailable(
            "head_changed",
            "The PR head changed while reading its Web conversation. Retry to obtain consistent comment positions.",
        ));
    }
    Ok(pr_comments::ReviewRead {
        comments,
        web_actor,
        head_commit_id: head.as_str().to_owned(),
    })
}

fn comment_section<T>(
    text: &mut String,
    label: &str,
    comments: Option<&[T]>,
    reason: Option<&str>,
) {
    use std::fmt::Write as _;
    match comments {
        Some(comments) => {
            let _ = writeln!(text, "--- {label} ({}) ---", comments.len());
        }
        None => {
            let reason = reason.unwrap_or("unknown cause");
            if reason == GbError::WebPasswordRequired.code() {
                let _ = writeln!(
                    text,
                    "--- {label}: unavailable ({}) ---",
                    GbError::WebPasswordRequired
                );
            } else {
                let _ = writeln!(text, "--- {label}: unavailable ({reason}) ---");
            }
        }
    }
}

fn format_comments(comments: &PrComments, selection: CommentSelection) -> String {
    let mut text = String::new();
    if selection.discussion() {
        comment_section(
            &mut text,
            "Comments",
            comments.comments.as_deref(),
            comments.comment_coverage.discussion.reason,
        );
        for comment in comments.comments.iter().flatten() {
            let id = CommentIdentity {
                instance: comments.comment_coverage.instance.clone(),
                kind: CommentKind::Discussion,
                repository: comments.comment_coverage.repository.clone(),
                id: comment.id,
            };
            text.push_str(&format!(
                "\n{id} {} ({})\n{}\n",
                comment
                    .user
                    .as_ref()
                    .map(|user| user.login.as_str())
                    .unwrap_or("unknown"),
                comment.created_at.as_deref().unwrap_or("unknown time"),
                comment.body.as_deref().unwrap_or("")
            ));
        }
    }
    if selection.review() {
        if !text.is_empty() {
            text.push('\n');
        }
        comment_section(
            &mut text,
            "Visible inline / commit review comments",
            comments.review_comments.as_deref(),
            comments.comment_coverage.review.reason,
        );
        for comment in comments.review_comments.iter().flatten() {
            let position = &comment.position;
            text.push_str(&format!(
                "\n{} {} ({}, server local timezone unknown)\n",
                comment.identity, comment.author, comment.created_at
            ));
            if let Some(path) = &position.path {
                text.push_str(&format!(
                    "File: {path}  old line: {}  new line: {}\n",
                    position
                        .old_line
                        .map(|n| n.to_string())
                        .unwrap_or_else(|| "unknown".into()),
                    position
                        .new_line
                        .map(|n| n.to_string())
                        .unwrap_or_else(|| "unknown".into())
                ));
            }
            text.push_str(&format!(
                "Commit: {}  outdated: {}\n{}\n{}\n",
                position.commit_id.as_deref().unwrap_or("unknown"),
                position
                    .outdated
                    .map(|v| v.to_string())
                    .unwrap_or_else(|| "unknown".into()),
                comment.body.text,
                comment.html_url
            ));
        }
    }
    text
}

pub(super) async fn view(
    hostname: &Option<String>,
    cli_repo: &Option<String>,
    cli_profile: &Option<String>,
    options: ViewOptions,
) -> Result<()> {
    let ctx = RepoContext::resolve(hostname, cli_repo, cli_profile)?;

    let selection = if options.web || !options.show_comments {
        CommentSelection::Discussion
    } else {
        options.selection
    };
    let (pr, head) = load_comment_target(&ctx, options.number, selection).await?;

    if options.web {
        let url = ctx.client.web_url(&format!(
            "/{}/{}/pull/{}",
            ctx.owner, ctx.repo, options.number
        ));
        return output::open_web_url(&url);
    }

    let comments = if options.show_comments {
        Some(load_comments(&ctx, cli_profile, options.number, selection, head.as_ref()).await?)
    } else {
        None
    };

    if options.json.enabled() {
        let mut value = serde_json::to_value(&pr)?;
        if let Some(comments) = &comments {
            output::comments::add_pr(&mut value, comments)?;
        }
        return options.json.page_value(value, options.no_pager);
    }

    let mut text = format_pr_view(&pr);
    if let Some(comments) = comments {
        text.push_str(&format!("\n{}", format_comments(&comments, selection)));
    }
    page_or_print(&text, options.no_pager)?;
    Ok(())
}

fn format_pr_view(pr: &PullRequest) -> String {
    let state = if pr.merged == Some(true) {
        "merged"
    } else if pr.state == "open" && pr.draft == Some(true) {
        "draft"
    } else {
        &pr.state
    };
    let mut output = String::new();
    output.push_str(&format!(
        "{} {}\n",
        output::single_line(&pr.title).bold(),
        format!("#{}", pr.number).dimmed()
    ));
    output.push_str(&format!("{}\n\n", format_state(state)));

    if let Some(head) = &pr.head {
        if let Some(base) = &pr.base {
            output.push_str(&format!(
                "{} ← {}\n",
                output::single_line(&base.ref_name).cyan(),
                output::single_line(&head.ref_name).green()
            ));
        }
    }

    if let Some(user) = &pr.user {
        output.push_str(&format!("Author: {}  ", output::single_line(&user.login)));
    }
    if let Some(created) = &pr.created_at {
        output.push_str(&format!("Created: {}", output::single_line(created)));
    }
    output.push('\n');

    if let Some(draft) = pr.draft {
        output.push_str(&format!("Draft: {}\n", if draft { "yes" } else { "no" }));
    }
    let head = pr
        .head
        .as_ref()
        .and_then(|head| head.sha.as_deref())
        .and_then(|sha| crate::domain::pull_request::CommitId::parse(sha).ok());
    output.push_str(&format!(
        "Head commit: {}\n",
        head.as_ref().map(|sha| sha.as_str()).unwrap_or("unknown")
    ));
    output.push_str(&format!(
        "Mergeable: {}\n",
        match pr.mergeable {
            Some(true) => "yes",
            Some(false) => "no",
            None => "unknown",
        }
    ));
    output.push_str("Review decision: unknown (approval status is not queried; comments do not establish approval)\n");
    if let Some(labels) = &pr.labels {
        let names: Vec<_> = labels
            .iter()
            .map(|label| output::single_line(&label.name))
            .collect();
        output.push_str(&format!(
            "Labels: {}\n",
            if names.is_empty() {
                "(none)".into()
            } else {
                names.join(", ")
            }
        ));
    }
    if let Some(assignees) = &pr.assignees {
        let names: Vec<_> = assignees
            .iter()
            .map(|user| output::single_line(&user.login))
            .collect();
        output.push_str(&format!(
            "Assignees: {}\n",
            if names.is_empty() {
                "(none)".into()
            } else {
                names.join(", ")
            }
        ));
    }
    if let Some(user) = &pr.merged_by {
        output.push_str(&format!(
            "Merged by: {}\n",
            output::single_line(&user.login)
        ));
    }

    if let Some(body) = &pr.body {
        if !body.is_empty() {
            output.push_str(&format!("\n{}\n", body));
        }
    }

    output
}

pub(super) fn print_pr_refs(pr: &PullRequest) {
    if let Some(head) = &pr.head {
        println!("Head: {}", format_pr_ref(head));
    }
    if let Some(base) = &pr.base {
        println!("Base: {}", format_pr_ref(base));
    }
}

fn format_pr_ref(pr_ref: &crate::models::pull_request::PullRequestHead) -> String {
    match &pr_ref.repo {
        Some(repo) => format!("{}:{}", repo.full_name, pr_ref.ref_name),
        None => pr_ref
            .label
            .clone()
            .unwrap_or_else(|| pr_ref.ref_name.clone()),
    }
}
