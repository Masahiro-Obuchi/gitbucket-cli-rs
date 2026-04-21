use std::process::Command;

use dialoguer::Password;

use crate::api::client::ApiClient;
use crate::api::web::GitBucketWebSession;
use crate::config::auth::{AuthConfig, HostConfig};
use crate::error::{GbError, Result};

/// Resolve GitBucket host or URL from CLI arg, env var, or config
pub fn resolve_hostname(
    cli_hostname: &Option<String>,
    cli_profile: &Option<String>,
) -> Result<String> {
    let config = AuthConfig::load()?;
    let hostname = config.resolve_hostname(cli_hostname.as_deref(), cli_profile.as_deref())?;
    let has_active_profile = config
        .active_profile_name(cli_profile.as_deref())?
        .is_some();
    hostname.ok_or_else(|| {
        if has_active_profile {
            GbError::Auth(
                "No GitBucket host or URL configured for the selected profile. Pass --hostname or set the profile default host.".into(),
            )
        } else {
            GbError::Auth("No GitBucket host or URL configured. Run `gb auth login` first.".into())
        }
    })
}

/// Validate a selected profile even for commands that do not otherwise need config.
pub fn validate_selected_profile(cli_profile: &Option<String>) -> Result<()> {
    let config = AuthConfig::load()?;
    config.active_profile_name(cli_profile.as_deref())?;
    Ok(())
}

/// Resolve owner/repo from CLI arg, env var, or git remote
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

/// Parse "OWNER/REPO" string
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

/// Normalize the state filter for list commands.
pub fn normalize_list_state(state: &str) -> Result<String> {
    match state.to_ascii_lowercase().as_str() {
        "open" | "closed" | "all" => Ok(state.to_ascii_lowercase()),
        _ => Err(GbError::Other(format!(
            "Invalid state '{}'. Expected one of: open, closed, all",
            state
        ))),
    }
}

/// Detect owner/repo from the current git remote
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

/// Parse a git remote URL to extract owner/repo
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

pub fn resolve_host_config(hostname: &str, cli_profile: &Option<String>) -> Result<HostConfig> {
    let config = AuthConfig::load()?;
    config.get_host_for_profile(hostname, cli_profile.as_deref())
}

/// Create an API client from config
pub fn create_client(hostname: &str, cli_profile: &Option<String>) -> Result<ApiClient> {
    let host = resolve_host_config(hostname, cli_profile)?;
    ApiClient::new(hostname, &host.token, &host.protocol)
}

pub async fn create_web_session(
    hostname: &str,
    cli_profile: &Option<String>,
) -> Result<GitBucketWebSession> {
    let host = resolve_host_config(hostname, cli_profile)?;
    let username = if let Ok(user) = std::env::var("GB_USER") {
        user
    } else if !host.user.is_empty() {
        host.user.clone()
    } else {
        return Err(GbError::Auth(
            "GitBucket web actions require a username. Run `gb auth login` first or set `GB_USER`."
                .into(),
        ));
    };

    let password = match std::env::var("GB_PASSWORD") {
        Ok(password) => password,
        Err(_) => Password::new()
            .with_prompt(format!("GitBucket password for {}", username))
            .interact()?,
    };

    GitBucketWebSession::sign_in(hostname, &username, &password, &host.protocol).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_owner_repo() {
        let (owner, repo) = parse_owner_repo("alice/my-repo").unwrap();
        assert_eq!(owner, "alice");
        assert_eq!(repo, "my-repo");
    }

    #[test]
    fn test_parse_owner_repo_invalid() {
        assert!(parse_owner_repo("noslash").is_err());
        assert!(parse_owner_repo("/repo").is_err());
        assert!(parse_owner_repo("owner/").is_err());
    }

    #[test]
    fn test_normalize_list_state() {
        assert_eq!(normalize_list_state("OPEN").unwrap(), "open");
        assert_eq!(normalize_list_state("closed").unwrap(), "closed");
        assert_eq!(normalize_list_state("all").unwrap(), "all");
        assert!(normalize_list_state("draft").is_err());
    }

    #[test]
    fn test_parse_git_url_https() {
        let (owner, repo) =
            parse_git_url("https://gitbucket.example.com/alice/my-repo.git").unwrap();
        assert_eq!(owner, "alice");
        assert_eq!(repo, "my-repo");
    }

    #[test]
    fn test_parse_git_url_ssh() {
        let (owner, repo) = parse_git_url("git@gitbucket.example.com:alice/my-repo.git").unwrap();
        assert_eq!(owner, "alice");
        assert_eq!(repo, "my-repo");
    }

    #[test]
    fn test_parse_git_url_with_git_prefix() {
        let (owner, repo) =
            parse_git_url("https://gitbucket.example.com/git/alice/my-repo.git").unwrap();
        assert_eq!(owner, "alice");
        assert_eq!(repo, "my-repo");
    }

    #[test]
    fn test_parse_git_url_with_subpath() {
        let (owner, repo) =
            parse_git_url("https://gitbucket.example.com/gitbucket/alice/my-repo.git").unwrap();
        assert_eq!(owner, "alice");
        assert_eq!(repo, "my-repo");
    }

    #[test]
    fn test_parse_git_url_with_subpath_and_git_prefix() {
        let (owner, repo) =
            parse_git_url("https://gitbucket.example.com/gitbucket/git/alice/my-repo.git").unwrap();
        assert_eq!(owner, "alice");
        assert_eq!(repo, "my-repo");
    }
}
