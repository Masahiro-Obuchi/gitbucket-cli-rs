use std::process::Command;

use crate::config::auth::AuthConfig;
use crate::error::{GbError, Result};

/// Resolve owner/repo from CLI arg, env var, or git remote.
pub fn resolve_repo(
    cli_repo: &Option<String>,
    cli_profile: &Option<String>,
) -> Result<(String, String)> {
    let config = AuthConfig::load()?;
    if let Some(repo) = config.resolve_repo(cli_repo.as_deref(), cli_profile.as_deref())? {
        return parse_owner_repo(&repo);
    }
    detect_repo_from_git()
}

/// Parse "OWNER/REPO" string.
pub fn parse_owner_repo(s: &str) -> Result<(String, String)> {
    let parts: Vec<&str> = s.splitn(2, '/').collect();
    if parts.len() != 2 || parts[0].is_empty() || parts[1].is_empty() {
        return Err(GbError::Other(format!(
            "Invalid repository format: '{}'. Expected OWNER/REPO",
            s
        )));
    }
    Ok((parts[0].to_string(), parts[1].to_string()))
}

/// Detect owner/repo from the current git remote.
fn detect_repo_from_git() -> Result<(String, String)> {
    let output = Command::new("git")
        .args(["remote", "get-url", "origin"])
        .output();

    let output = match output {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).trim().to_string(),
        _ => return Err(GbError::RepoNotFound),
    };

    parse_git_url(&output)
}

/// Parse a git remote URL to extract owner/repo.
pub(crate) fn parse_git_url(url: &str) -> Result<(String, String)> {
    let path = if let Some(rest) = url.strip_prefix("git@") {
        rest.split(':').nth(1).unwrap_or("").to_string()
    } else if url.starts_with("http://") || url.starts_with("https://") {
        let parsed = url::Url::parse(url).map_err(|_| GbError::RepoNotFound)?;
        let path = parsed.path().trim_start_matches('/').to_string();
        path.strip_prefix("git/").unwrap_or(&path).to_string()
    } else {
        return Err(GbError::RepoNotFound);
    };

    parse_repo_path(&path)
}

fn parse_repo_path(path: &str) -> Result<(String, String)> {
    let segments: Vec<&str> = path
        .trim_matches('/')
        .split('/')
        .filter(|segment| !segment.is_empty())
        .collect();

    if segments.len() < 2 {
        return Err(GbError::RepoNotFound);
    }

    let owner = segments[segments.len() - 2];
    let repo = segments[segments.len() - 1].trim_end_matches(".git");

    if owner.is_empty() || repo.is_empty() {
        return Err(GbError::RepoNotFound);
    }

    Ok((owner.to_string(), repo.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_owner_repo_accepts_owner_repo() {
        let (owner, repo) = parse_owner_repo("alice/my-repo").unwrap();
        assert_eq!(owner, "alice");
        assert_eq!(repo, "my-repo");
    }

    #[test]
    fn parse_owner_repo_rejects_invalid_values() {
        assert!(parse_owner_repo("noslash").is_err());
        assert!(parse_owner_repo("/repo").is_err());
        assert!(parse_owner_repo("owner/").is_err());
    }

    #[test]
    fn parse_git_url_https() {
        let (owner, repo) =
            parse_git_url("https://gitbucket.example.com/alice/my-repo.git").unwrap();
        assert_eq!(owner, "alice");
        assert_eq!(repo, "my-repo");
    }

    #[test]
    fn parse_git_url_ssh() {
        let (owner, repo) = parse_git_url("git@gitbucket.example.com:alice/my-repo.git").unwrap();
        assert_eq!(owner, "alice");
        assert_eq!(repo, "my-repo");
    }

    #[test]
    fn parse_git_url_with_git_prefix() {
        let (owner, repo) =
            parse_git_url("https://gitbucket.example.com/git/alice/my-repo.git").unwrap();
        assert_eq!(owner, "alice");
        assert_eq!(repo, "my-repo");
    }

    #[test]
    fn parse_git_url_with_subpath() {
        let (owner, repo) =
            parse_git_url("https://gitbucket.example.com/gitbucket/alice/my-repo.git").unwrap();
        assert_eq!(owner, "alice");
        assert_eq!(repo, "my-repo");
    }

    #[test]
    fn parse_git_url_with_subpath_and_git_prefix() {
        let (owner, repo) =
            parse_git_url("https://gitbucket.example.com/gitbucket/git/alice/my-repo.git").unwrap();
        assert_eq!(owner, "alice");
        assert_eq!(repo, "my-repo");
    }
}
