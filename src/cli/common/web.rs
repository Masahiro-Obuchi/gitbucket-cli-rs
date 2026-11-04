use dialoguer::Password;

use crate::api::client::ApiClient;
use crate::api::web::GitBucketWebSession;
use crate::error::{GbError, Result};
use crate::models::issue::Issue;

use super::resolve_host_config;

pub async fn create_web_session(
    client: &ApiClient,
    hostname: &str,
    cli_profile: &Option<String>,
) -> Result<GitBucketWebSession> {
    let host = resolve_host_config(hostname, cli_profile)?;
    let explicit_web_user = std::env::var("GB_WEB_USER").ok();
    let username = if let Some(user) = &explicit_web_user {
        user.clone()
    } else if let Ok(user) = std::env::var("GB_USER") {
        user
    } else if !host.user.is_empty() {
        host.user.clone()
    } else {
        return Err(GbError::Auth(
            "GitBucket web actions require a username. Run `gb auth login` first or set `GB_USER`."
                .into(),
        ));
    };

    crate::target::validate_component(&username)?;
    let api_user = client.current_user().await?;
    if explicit_web_user.is_none() && !api_user.login.eq_ignore_ascii_case(&username) {
        return Err(GbError::Auth(format!("REST token belongs to {:?}, but Web username is {:?}. No Web changes were attempted. Set GB_WEB_USER explicitly to authorize a different Web actor.", api_user.login, username)));
    }
    let password = match std::env::var("GB_PASSWORD") {
        Ok(password) => password,
        Err(_) => Password::new()
            .with_prompt(format!("GitBucket password for {}", username))
            .interact()?,
    };

    let session =
        GitBucketWebSession::sign_in(hostname, &username, &password, &host.protocol).await?;
    eprintln!(
        "Notice: Web actor {}; REST actor {}.",
        session.principal(),
        api_user.login
    );
    Ok(session)
}

pub async fn update_issue_assignees_via_web(
    client: &ApiClient,
    session: &GitBucketWebSession,
    owner: &str,
    repo: &str,
    number: u64,
    current: &Issue,
    next: &[String],
) -> Result<()> {
    use crate::operation::{Progress, Route};
    let current: Vec<String> = current
        .assignees
        .iter()
        .map(|assignee| assignee.login.clone())
        .collect();

    let removals: Vec<_> = current
        .iter()
        .filter(|assignee| !next.iter().any(|value| value == *assignee))
        .map(|name| ("remove", name))
        .collect();
    let additions = next
        .iter()
        .filter(|assignee| !current.iter().any(|value| value == *assignee))
        .map(|name| ("add", name));
    let changes: Vec<_> = removals.into_iter().chain(additions).collect();
    let mut progress = Progress::default();
    let steps: Vec<_> = changes
        .iter()
        .map(|(action, name)| progress.plan(format!("assignee.{action}:{name}"), Route::Web))
        .collect();
    progress.actor(session.principal());
    let last = steps.last().copied();
    for ((action, assignee), step) in changes.into_iter().zip(steps) {
        progress
            .run(
                step,
                session.update_issue_assignee(owner, repo, number, action, assignee),
                |_| async {
                    let actual = client.get_issue(owner, repo, number).await?;
                    let present = actual.assignees.iter().any(|user| user.login == *assignee);
                    let exact = Some(step) != last
                        || actual
                            .assignees
                            .iter()
                            .map(|u| &u.login)
                            .collect::<std::collections::BTreeSet<_>>()
                            == next.iter().collect();
                    if actual.number != number || present != (action == "add") || !exact {
                        return Err(GbError::Other(format!(
                            "Stored assignees do not confirm {action} {assignee:?}."
                        )));
                    }
                    Ok(())
                },
            )
            .await?;
    }

    Ok(())
}

pub async fn preflight_assignees(
    client: &ApiClient,
    current: &Issue,
    next: &[String],
) -> Result<()> {
    for name in next {
        crate::target::validate_component(name)?;
        if !current.assignees.iter().any(|user| user.login == *name) {
            let user: crate::models::user::User = client.get(&format!("/users/{name}")).await?;
            if user.login != *name {
                return Err(GbError::Other(
                    "Assignee lookup returned another user. No changes were applied.".into(),
                ));
            }
        }
    }
    Ok(())
}
