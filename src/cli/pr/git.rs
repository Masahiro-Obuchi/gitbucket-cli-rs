use crate::git::GitCommandExt;
use std::process::{Command, Output};

use crate::cli::common::RepoContext;
use crate::error::{GbError, Result};
use crate::git::GitAuth;
use crate::models::pull_request::PullRequest;
use crate::models::repository::Repository;
use crate::output;

pub(super) fn git_remote_names() -> Result<Vec<String>> {
    let output = Command::new("git").args(["remote"]).bounded_output()?;
    if !output.status.success() {
        return Ok(Vec::new());
    }
    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(ToOwned::to_owned)
        .collect())
}

pub(super) fn git_remote_url(remote: &str) -> Result<Option<String>> {
    let output = Command::new("git")
        .args(["config", "--get", &format!("remote.{remote}.url")])
        .bounded_output()?;
    let url = String::from_utf8_lossy(&output.stdout).trim().to_string();
    Ok((output.status.success() && !url.is_empty()).then_some(url))
}

#[derive(Debug, Clone)]
pub(super) struct GitFetchSource {
    pub command_source: String,
    pub display_source: String,
}

pub(super) fn resolve_git_fetch_source(source: &str) -> Result<GitFetchSource> {
    let resolved = git_remote_url(source)?.unwrap_or_else(|| source.to_string());
    let clean = crate::git::clean_source(&resolved)?;
    Ok(GitFetchSource {
        command_source: clean.clone(),
        display_source: clean,
    })
}

pub(super) fn matching_remote_name(repo: &Repository) -> Result<Option<String>> {
    let Some(expected_url) = repo.clone_url.as_deref() else {
        return Ok(None);
    };

    for remote in git_remote_names()? {
        let Some(url) = git_remote_url(&remote)? else {
            continue;
        };

        if crate::git::repository_urls_match(expected_url, &url) {
            return Ok(Some(remote));
        }
    }

    Ok(None)
}

fn pr_repo_fetch_source(repo: Option<&Repository>, auth: &crate::git::GitAuth) -> Result<String> {
    let Some(repo) = repo else {
        return Ok("origin".to_string());
    };
    let clone_url = auth.clone_url(repo);
    let mut resolved = repo.clone();
    resolved.clone_url = Some(clone_url.clone());
    Ok(matching_remote_name(&resolved)?.unwrap_or(clone_url))
}

pub(super) fn pr_head_fetch_source(pr: &PullRequest, auth: &crate::git::GitAuth) -> Result<String> {
    pr_repo_fetch_source(pr.head.as_ref().and_then(|head| head.repo.as_ref()), auth)
}

pub(super) fn pr_base_fetch_source(pr: &PullRequest, auth: &crate::git::GitAuth) -> Result<String> {
    pr_repo_fetch_source(pr.base.as_ref().and_then(|base| base.repo.as_ref()), auth)
}

pub(super) fn current_branch_name() -> Result<Option<String>> {
    let output = Command::new("git")
        .args(["branch", "--show-current"])
        .bounded_output()?;
    let branch = String::from_utf8_lossy(&output.stdout).trim().to_string();
    Ok((output.status.success() && !branch.is_empty()).then_some(branch))
}

pub(super) fn target_identity(ctx: &RepoContext, number: u64) -> String {
    // Length-delimited JSON avoids collisions between deployment/repo boundaries.
    serde_json::to_string(&(ctx.client.web_url(""), &ctx.owner, &ctx.repo, number)).unwrap()
}

pub(super) fn fetch_namespace(identity: &str) -> String {
    // Keep ref paths bounded even for long deployment URLs on Windows. The full
    // identity remains in branch provenance; this digest names only fetched refs.
    let digest = ring::digest::digest(&ring::digest::SHA256, identity.as_bytes());
    let encoded: String = digest
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    format!("refs/gb/pr-targets/v1/{encoded}")
}

pub(super) fn git_fetch_ref(
    auth: &GitAuth,
    source: &str,
    branch: &str,
    target_ref: &str,
) -> Result<Output> {
    let valid = Command::new("git")
        .args(["check-ref-format", &format!("refs/heads/{branch}")])
        .bounded_output()?;
    if !valid.status.success() {
        return Err(GbError::Other(
            "API returned an invalid PR branch ref.".into(),
        ));
    }
    let mut output = auth
        .command(source)?
        .arg("fetch")
        .args([
            "--no-tags",
            "--no-write-fetch-head",
            "--no-recurse-submodules",
            "--refmap=",
            "--",
        ])
        .arg(source)
        .arg(format!("+refs/heads/{branch}:{target_ref}"))
        .bounded_output()?;
    auth.redact_output(&mut output);
    Ok(output)
}

pub(super) fn print_command_output(output: &Output) {
    if !output.stdout.is_empty() {
        print!("{}", String::from_utf8_lossy(&output.stdout));
    }
    if !output.stderr.is_empty() && !output::suppress_stderr() {
        eprint!("{}", String::from_utf8_lossy(&output.stderr));
    }
}

pub(super) fn command_stderr(output: &Output) -> String {
    let stderr = String::from_utf8_lossy(&output.stderr);
    let trimmed = stderr.trim();
    if trimmed.is_empty() {
        return "git did not provide stderr details.".into();
    }
    trimmed.lines().take(3).collect::<Vec<_>>().join(" ")
}
