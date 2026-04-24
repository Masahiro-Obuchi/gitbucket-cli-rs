use std::process::Command;

use dialoguer::Password;

use crate::api::client::ApiClient;
use crate::api::web::GitBucketWebSession;
use crate::config::auth::{AuthConfig, HostConfig};
use crate::error::{GbError, Result};
use crate::models::issue::Issue;

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

/// Normalize an issue-like edit state.
pub fn normalize_edit_state(kind: &str, state: Option<String>) -> Result<Option<String>> {
    match state {
        None => Ok(None),
        Some(value) => match value.to_ascii_lowercase().as_str() {
            "open" | "closed" => Ok(Some(value.to_ascii_lowercase())),
            _ => Err(GbError::Other(format!(
                "Invalid {} state. Expected 'open' or 'closed'.",
                kind
            ))),
        },
    }
}

/// Normalize repeated and comma-delimited string arguments.
pub fn normalize_str_vec(values: Vec<String>) -> Vec<String> {
    values
        .into_iter()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
        .collect()
}

/// Apply remove operations first, then append missing additions.
pub fn merge_named_values(
    current: impl IntoIterator<Item = String>,
    additions: Vec<String>,
    removals: Vec<String>,
) -> Vec<String> {
    let mut values: Vec<String> = current.into_iter().collect();
    values.retain(|value| !removals.iter().any(|removed| removed == value));
    for addition in additions {
        if !values.iter().any(|existing| existing == &addition) {
            values.push(addition);
        }
    }
    values
}

pub async fn update_issue_assignees_via_web(
    session: &GitBucketWebSession,
    owner: &str,
    repo: &str,
    number: u64,
    current: &Issue,
    next: &[String],
) -> Result<()> {
    let current: Vec<String> = current
        .assignees
        .iter()
        .map(|assignee| assignee.login.clone())
        .collect();

    for assignee in current
        .iter()
        .filter(|assignee| !next.iter().any(|value| value == *assignee))
    {
        session
            .update_issue_assignee(owner, repo, number, "remove", assignee)
            .await?;
    }

    for assignee in next
        .iter()
        .filter(|assignee| !current.iter().any(|value| value == *assignee))
    {
        session
            .update_issue_assignee(owner, repo, number, "add", assignee)
            .await?;
    }

    Ok(())
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
    fn normalize_edit_state_accepts_open_and_closed() {
        assert_eq!(
            normalize_edit_state("issue", Some("open".into())).unwrap(),
            Some("open".into())
        );
        assert_eq!(
            normalize_edit_state("issue", Some("Closed".into())).unwrap(),
            Some("closed".into())
        );
    }

    #[test]
    fn normalize_edit_state_rejects_other_values() {
        assert!(normalize_edit_state("issue", Some("all".into())).is_err());
    }

    #[test]
    fn merge_named_values_applies_removals_then_additions() {
        let merged = merge_named_values(
            vec!["bug".into(), "urgent".into()],
            vec!["enhancement".into(), "urgent".into()],
            vec!["bug".into()],
        );

        assert_eq!(merged, vec!["urgent", "enhancement"]);
    }

    #[test]
    fn normalize_str_vec_trims_whitespace_and_drops_empty() {
        assert_eq!(
            normalize_str_vec(vec!["bug".into(), " urgent".into(), "".into()]),
            vec!["bug", "urgent"]
        );
        assert_eq!(
            normalize_str_vec(vec!["  alice  ".into(), "  ".into(), "bob".into()]),
            vec!["alice", "bob"]
        );
        assert_eq!(
            normalize_str_vec(vec!["".into(), "  ".into()]),
            Vec::<String>::new()
        );
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
