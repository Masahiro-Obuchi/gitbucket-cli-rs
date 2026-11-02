use std::process::Command;

use crate::config::auth::AuthConfig;
use crate::error::{GbError, Result};

/// Resolve an explicit name, or validate origin against the configured instance.
pub(super) fn resolve_target(
    config: &AuthConfig,
    hostname: &str,
    protocol: &str,
    cli_repo: &Option<String>,
    cli_profile: &Option<String>,
) -> Result<crate::target::RepositoryTarget> {
    let instance = url::Url::parse(&crate::api::client::normalize_web_base_url(
        hostname, protocol,
    )?)?;
    if let Some(repo) = config.resolve_repo(cli_repo.as_deref(), cli_profile.as_deref())? {
        return crate::target::RepositoryTarget::explicit(instance, &repo);
    }
    let output = Command::new("git")
        .args(["remote", "get-url", "origin"])
        .output();
    let output = match output {
        Ok(output) if output.status.success() => output,
        _ => return Err(GbError::RepoNotFound),
    };
    let remote = String::from_utf8_lossy(&output.stdout);
    crate::target::RepositoryTarget::from_remote(instance, remote.trim())
}

/// Parse a repository name without permitting URL delimiters or traversal.
pub fn parse_owner_repo(value: &str) -> Result<(String, String)> {
    let name = crate::target::RepositoryName::parse(value)?;
    Ok((name.owner, name.repo))
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
}
