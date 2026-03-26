use crate::cli::common::{parse_git_url, parse_owner_repo};
use crate::config::auth::AuthConfig;
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

fn credentialed_git_http_url(hostname: &str, url: &str) -> Option<String> {
    if !url.starts_with("http://") && !url.starts_with("https://") {
        return None;
    }

    let config = AuthConfig::load().ok()?;
    let host = config.get_host(hostname).ok()?;
    if host.user.is_empty() {
        return None;
    }

    let mut parsed = url::Url::parse(url).ok()?;
    parsed.set_username(&host.user).ok()?;
    parsed.set_password(Some(&host.token)).ok()?;
    Some(parsed.to_string())
}

pub(super) fn resolve_git_fetch_source(hostname: &str, source: &str) -> GitFetchSource {
    if source.starts_with("http://") || source.starts_with("https://") {
        return GitFetchSource {
            command_source: credentialed_git_http_url(hostname, source)
                .unwrap_or_else(|| source.to_string()),
            display_source: source.to_string(),
        };
    }

    if let Some(remote_url) = git_remote_url(source) {
        if let Some(command_source) = credentialed_git_http_url(hostname, &remote_url) {
            return GitFetchSource {
                command_source,
                display_source: source.to_string(),
            };
        }
    }

    GitFetchSource {
        command_source: source.to_string(),
        display_source: source.to_string(),
    }
}

pub(super) fn matching_remote_name(repo: &Repository) -> Option<String> {
    let expected_full_name = parse_owner_repo(&repo.full_name).ok();

    for remote in git_remote_names() {
        let Some(url) = git_remote_url(&remote) else {
            continue;
        };

        if repo.clone_url.as_deref() == Some(url.as_str()) {
            return Some(remote);
        }

        if let (Some((expected_owner, expected_repo)), Ok((owner, repo_name))) =
            (expected_full_name.as_ref(), parse_git_url(&url))
        {
            if owner == *expected_owner && repo_name == *expected_repo {
                return Some(remote);
            }
        }
    }

    None
}

fn pr_repo_fetch_source(repo: Option<&Repository>) -> String {
    repo.and_then(matching_remote_name)
        .or_else(|| repo.and_then(|repository| repository.clone_url.clone()))
        .unwrap_or_else(|| "origin".to_string())
}

pub(super) fn pr_head_fetch_source(pr: &PullRequest) -> String {
    pr_repo_fetch_source(pr.head.as_ref().and_then(|head| head.repo.as_ref()))
}

pub(super) fn pr_base_fetch_source(pr: &PullRequest) -> String {
    pr_repo_fetch_source(pr.base.as_ref().and_then(|base| base.repo.as_ref()))
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
