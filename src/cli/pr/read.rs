use colored::Colorize;

use crate::application::pr_comments::{self, PrComments};
use crate::cli::common::{create_web_session, RepoContext};
use crate::domain::review_comment::{CommentIdentity, CommentKind, CommentSelection};
use crate::error::Result;
use crate::models::pull_request::PullRequest;
use crate::output;
use crate::output::table::format_table;
use crate::output::{format_state, page_or_print, truncate};

pub(super) struct ViewOptions {
    pub number: u64,
    pub show_comments: bool,
    pub selection: CommentSelection,
    pub web: bool,
    pub json: bool,
    pub no_pager: bool,
}

pub(super) async fn list(
    hostname: &Option<String>,
    cli_repo: &Option<String>,
    cli_profile: &Option<String>,
    state: &str,
    json: bool,
    no_pager: bool,
) -> Result<()> {
    let ctx = RepoContext::resolve(hostname, cli_repo, cli_profile)?;
    let state = crate::cli::common::normalize_list_state(state)?;

    let prs = ctx
        .client
        .list_repository_pull_requests(&ctx.owner, &ctx.repo, &state, json)
        .await?;

    if json {
        return output::page_json(&prs, no_pager);
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
    json: bool,
    no_pager: bool,
) -> Result<()> {
    let ctx = RepoContext::resolve(hostname, cli_repo, cli_profile)?;
    let (_, head) = load_comment_target(&ctx, number, selection).await?;
    let comments = load_comments(&ctx, cli_profile, number, selection, head.as_ref()).await?;
    if json {
        if selection == CommentSelection::Discussion {
            return output::page_json(&comments.comments, no_pager);
        }
        return output::page_json(&comments, no_pager);
    }
    page_or_print(&format_comments(&comments, selection), no_pager)?;
    Ok(())
}

async fn load_comment_target(
    ctx: &RepoContext,
    number: u64,
    selection: CommentSelection,
) -> Result<(PullRequest, Option<crate::domain::pull_request::CommitId>)> {
    if selection.review() {
        let (pr, head) =
            pr_comments::review_target(&ctx.client, &ctx.owner, &ctx.repo, number).await?;
        Ok((pr, Some(head)))
    } else {
        Ok((
            super::target::require_pull_request(ctx, number).await?,
            None,
        ))
    }
}
async fn load_comments(
    ctx: &RepoContext,
    profile: &Option<String>,
    number: u64,
    selection: CommentSelection,
    head: Option<&crate::domain::pull_request::CommitId>,
) -> Result<PrComments> {
    let reviews = if selection.review() {
        let head = head.ok_or_else(|| {
            crate::api::web::review::unavailable(
                "missing_head_identity",
                "Review comments require the saved head identity.",
            )
        })?;
        eprintln!("Notice: Reading inline/commit reviews from the GitBucket 4.44 Web conversation; public/token-readable pages need no Web password.");
        let (comments, web_actor) = match ctx
            .client
            .token_review_comments(&ctx.owner, &ctx.repo, number, head)
            .await
        {
            Ok(comments) => (comments, None),
            Err(crate::error::GbError::Api {
                status: 401 | 403, ..
            }) => {
                eprintln!("Notice: Web read was denied; cookie-authenticated sign-in is required for this review page.");
                let session = create_web_session(&ctx.client, &ctx.hostname, profile).await?;
                let comments = session
                    .review_comments(&ctx.owner, &ctx.repo, number, head)
                    .await?;
                (comments, Some(session.principal().to_owned()))
            }
            Err(error) => return Err(error),
        };
        Some(pr_comments::ReviewRead {
            comments,
            web_actor,
        })
    } else {
        eprintln!("Notice: Scope is ordinary discussion comments only; inline/commit reviews are not requested. Use --kind all for Web review reads.");
        None
    };
    let comments = pr_comments::read(
        &ctx.client,
        reviews,
        &ctx.owner,
        &ctx.repo,
        number,
        selection,
        head,
    )
    .await?;
    if selection.review() {
        eprintln!("Notice: Review scope is the visible PR conversation. GitBucket may omit historical reviews; original commit/line and reply relationships can be unknown. Bodies are rendered HTML/plain text, not original Markdown.");
    }
    Ok(comments)
}

fn format_comments(comments: &PrComments, selection: CommentSelection) -> String {
    let mut text = String::new();
    if selection.discussion() {
        text.push_str("--- Comments ---\nScope: ordinary discussion only\n");
        for comment in &comments.comments {
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
        text.push_str("\n--- Inline / commit review comments ---\nScope: visible PR conversation; historical completeness unknown\nBody: rendered text; original Markdown / original positions may be unknown\nPositions: GitBucket Web display; not validated against Git\n");
        for comment in &comments.review_comments {
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

    let selection = if options.web {
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
    if options.json {
        let value = if let Some(comments) = &comments {
            let mut value = output::comments::pull_request(&pr, &comments.comments)?;
            if selection.review() {
                output::comments::add_reviews(&mut value, comments)?;
            }
            value
        } else {
            serde_json::to_value(&pr)?
        };
        return output::page_json(&value, options.no_pager);
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
        pr.title.bold(),
        format!("#{}", pr.number).dimmed()
    ));
    output.push_str(&format!("{}\n\n", format_state(state)));

    if let Some(head) = &pr.head {
        if let Some(base) = &pr.base {
            output.push_str(&format!(
                "{} ← {}\n",
                base.ref_name.cyan(),
                head.ref_name.green()
            ));
        }
    }

    if let Some(user) = &pr.user {
        output.push_str(&format!("Author: {}  ", user.login));
    }
    if let Some(created) = &pr.created_at {
        output.push_str(&format!("Created: {}", created));
    }
    output.push('\n');

    if let Some(draft) = pr.draft {
        output.push_str(&format!("Draft: {}\n", if draft { "yes" } else { "no" }));
    }
    if let Some(labels) = &pr.labels {
        let names: Vec<_> = labels.iter().map(|label| label.name.as_str()).collect();
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
        let names: Vec<_> = assignees.iter().map(|user| user.login.as_str()).collect();
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
        output.push_str(&format!("Merged by: {}\n", user.login));
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
