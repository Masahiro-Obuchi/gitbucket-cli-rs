use crate::git::GitCommandExt;
use std::process::{Command, Output};

use crate::cli::common::RepoContext;
use crate::error::{GbError, Result};

use super::git::{
    command_stderr, fetch_namespace, git_fetch_ref, pr_base_fetch_source, pr_head_fetch_source,
    resolve_git_fetch_source, target_identity,
};

pub(super) async fn diff(
    hostname: &Option<String>,
    cli_repo: &Option<String>,
    cli_profile: &Option<String>,
    number: u64,
    no_pager: bool,
) -> Result<()> {
    let ctx = RepoContext::resolve(hostname, cli_repo, cli_profile)?;

    let pr = super::target::require_pull_request(&ctx, number).await?;
    match live_branch_diff(&ctx, number, no_pager, &pr) {
        Ok(diff) => {
            print!("{diff}");
            Ok(())
        }
        Err(live_error) => {
            match saved_pr_diff(&ctx.client, &ctx.owner, &ctx.repo, number, &pr).await {
                Ok(diff) => {
                    print!("{diff}");
                    Ok(())
                }
                Err(saved_error) => Err(GbError::DiffUnavailable {
                    number,
                    cause: live_error.cause,
                    message: format!("{} {}", live_error.message, saved_error),
                }),
            }
        }
    }
}

struct DiffFailure {
    cause: &'static str,
    message: String,
}

fn live_branch_diff(
    ctx: &RepoContext,
    number: u64,
    no_pager: bool,
    pr: &crate::models::pull_request::PullRequest,
) -> std::result::Result<String, DiffFailure> {
    let auth = &ctx.git_auth;
    let head = pr
        .head
        .as_ref()
        .map(|h| h.ref_name.as_str())
        .ok_or_else(|| DiffFailure {
            cause: "missing_head_ref",
            message: "API response does not include a head ref.".into(),
        })?;
    let base = pr
        .base
        .as_ref()
        .map(|b| b.ref_name.as_str())
        .ok_or_else(|| DiffFailure {
            cause: "missing_base_ref",
            message: "API response does not include a base ref.".into(),
        })?;

    let fetch_source = pr_head_fetch_source(pr, auth)
        .and_then(|source| resolve_git_fetch_source(&source))
        .map_err(|err| DiffFailure {
            cause: "head_fetch_failed",
            message: err.to_string(),
        })?;
    let base_fetch_source = pr_base_fetch_source(pr, auth)
        .and_then(|source| resolve_git_fetch_source(&source))
        .map_err(|err| DiffFailure {
            cause: "base_fetch_failed",
            message: err.to_string(),
        })?;
    let namespace = fetch_namespace(&target_identity(ctx, number));
    let base_ref = format!("{namespace}/base");
    let head_ref = format!("{namespace}/head");

    let base_fetch = git_fetch_ref(auth, &base_fetch_source.command_source, base, &base_ref)
        .map_err(|err| DiffFailure {
            cause: "base_fetch_failed",
            message: format!("failed to run git fetch for base branch '{base}': {err}"),
        })?;
    if !base_fetch.status.success() {
        return Err(DiffFailure {
            cause: "base_fetch_failed",
            message: format!(
                "base branch '{}' could not be fetched from {}. {}",
                base,
                base_fetch_source.display_source,
                command_stderr(&base_fetch)
            ),
        });
    }

    let head_fetch =
        git_fetch_ref(auth, &fetch_source.command_source, head, &head_ref).map_err(|err| {
            DiffFailure {
                cause: "head_fetch_failed",
                message: format!("failed to run git fetch for head branch '{head}': {err}"),
            }
        })?;
    if !head_fetch.status.success() {
        return Err(DiffFailure {
            cause: "head_fetch_failed",
            message: format!(
                "head branch '{}' could not be fetched from {}; it may have been deleted or the fork may be inaccessible. {}",
                head,
                fetch_source.display_source,
                command_stderr(&head_fetch)
            ),
        });
    }

    let status = git_diff_refs(&base_ref, &head_ref, no_pager).map_err(|err| DiffFailure {
        cause: "git_diff_failed",
        message: format!("failed to run git diff: {err}"),
    })?;

    if !status.status.success() {
        return Err(DiffFailure {
            cause: "git_diff_failed",
            message: format!("git diff failed. {}", command_stderr(&status)),
        });
    }

    if status.stdout.is_empty() && pr.state != "open" {
        return Err(DiffFailure {
            cause: "empty_live_diff",
            message: format!(
                "fetched base and head refs for {} PR have no diff; the source branch may have been merged, deleted, or no longer represent the PR changes.",
                pr.state
            ),
        });
    }

    if !status.stderr.is_empty() {
        eprint!("{}", String::from_utf8_lossy(&status.stderr));
    }

    let output = String::from_utf8_lossy(&status.stdout).to_string();
    Ok(output)
}

async fn saved_pr_diff(
    client: &crate::api::client::ApiClient,
    owner: &str,
    repo: &str,
    number: u64,
    pr: &crate::models::pull_request::PullRequest,
) -> std::result::Result<String, String> {
    let mut urls = Vec::new();
    push_url(&mut urls, pr.diff_url.as_deref());
    push_url(&mut urls, pr.patch_url.as_deref());

    if urls.is_empty() {
        if let Ok(issue) = client.get_issue(owner, repo, number).await {
            if let Some(pull_request) = issue.pull_request {
                push_url(&mut urls, pull_request.diff_url.as_deref());
                push_url(&mut urls, pull_request.patch_url.as_deref());
            }
        }
    }

    let derived_diff_url = client.web_url(&format!("/{owner}/{repo}/pull/{number}.diff"));
    push_url(&mut urls, Some(&derived_diff_url));

    let mut failures = Vec::new();
    for (index, url) in urls.into_iter().enumerate() {
        // Candidate URLs can carry signed query credentials. Transport errors
        // already contain sanitized context; do not reattach the original URL.
        let candidate = index + 1;
        match client.get_text_from_origin(&url).await {
            Ok(body) if is_diff_body(&body) => return Ok(body),
            Ok(body) if body.trim().is_empty() => failures.push(format!(
                "Saved diff candidate {candidate}: empty saved diff"
            )),
            Ok(_) => failures.push(format!(
                "Saved diff candidate {candidate}: response was not a diff"
            )),
            Err(err) => failures.push(format!("Saved diff candidate {candidate}: {err}")),
        }
    }

    Err(format!(
        "No saved PR diff could be fetched from GitBucket. {}",
        failures.join("; ")
    ))
}

fn is_diff_body(body: &str) -> bool {
    body.lines().any(|line| {
        line.starts_with("diff --git ")
            || line.starts_with("--- ")
            || line.starts_with("+++ ")
            || line.starts_with("@@ ")
    })
}

fn push_url(urls: &mut Vec<String>, url: Option<&str>) {
    let Some(url) = url else {
        return;
    };
    if !urls.iter().any(|existing| existing == url) {
        urls.push(url.to_string());
    }
}

fn git_diff_refs(base_ref: &str, head_ref: &str, no_pager: bool) -> Result<Output> {
    let mut command = Command::new("git");
    if no_pager {
        command.env("GIT_PAGER", "cat");
        command.arg("--no-pager");
    }

    command
        .arg("diff")
        .arg(format!("{base_ref}...{head_ref}"))
        .bounded_output()
}
