use crate::error::Result;
use crate::models::pull_request::PullRequest;
use crate::models::repository::Repository;

pub(super) fn git_remote_names() -> Vec<String> {
    let output = match std::process::Command::new("git").args(["remote"]).output() {
        Ok(output) if output.status.success() => output,
        _ => return Vec::new(),
    };

    String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

pub(super) fn git_remote_url(remote: &str) -> Option<String> {
    let output = std::process::Command::new("git")
        .args(["config", "--get", &format!("remote.{}.url", remote)])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }

    let url = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if url.is_empty() {
        None
    } else {
        Some(url)
    }
}

#[derive(Debug, Clone)]
pub(super) struct GitFetchSource {
    pub command_source: String,
    pub display_source: String,
}

pub(super) fn resolve_git_fetch_source(source: &str) -> Result<GitFetchSource> {
    let resolved = git_remote_url(source).unwrap_or_else(|| source.to_string());
    let clean = crate::git::clean_source(&resolved)?;
    Ok(GitFetchSource {
        command_source: clean.clone(),
        display_source: clean,
    })
}

pub(super) fn matching_remote_name(repo: &Repository) -> Option<String> {
    let expected_url = repo.clone_url.as_deref()?;

    for remote in git_remote_names() {
        let Some(url) = git_remote_url(&remote) else {
            continue;
        };

        if crate::git::repository_urls_match(expected_url, &url) {
            return Some(remote);
        }
    }

    None
}

fn pr_repo_fetch_source(repo: Option<&Repository>, auth: &crate::git::GitAuth) -> String {
    let Some(repo) = repo else {
        return "origin".to_string();
    };
    let clone_url = auth.clone_url(repo);
    let mut resolved = repo.clone();
    resolved.clone_url = Some(clone_url.clone());
    matching_remote_name(&resolved).unwrap_or(clone_url)
}

pub(super) fn pr_head_fetch_source(pr: &PullRequest, auth: &crate::git::GitAuth) -> String {
    pr_repo_fetch_source(pr.head.as_ref().and_then(|head| head.repo.as_ref()), auth)
}

pub(super) fn pr_base_fetch_source(pr: &PullRequest, auth: &crate::git::GitAuth) -> String {
    pr_repo_fetch_source(pr.base.as_ref().and_then(|base| base.repo.as_ref()), auth)
}

pub(super) fn current_branch_name() -> Option<String> {
    let output = std::process::Command::new("git")
        .args(["branch", "--show-current"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }

    let branch = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if branch.is_empty() {
        None
    } else {
        Some(branch)
    }
}
