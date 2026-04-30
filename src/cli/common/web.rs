use dialoguer::Password;

use crate::api::web::GitBucketWebSession;
use crate::error::{GbError, Result};
use crate::models::issue::Issue;

use super::resolve_host_config;

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
