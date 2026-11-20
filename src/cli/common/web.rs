use dialoguer::Password;

use crate::api::client::ApiClient;
use crate::api::web::GitBucketWebSession;
use crate::config::{auth::ResolvedCredential, web_password::Key};
use crate::error::{GbError, Result};
use crate::models::issue::Issue;

use super::resolve_credential;

pub async fn create_web_session(
    client: &ApiClient,
    hostname: &str,
    cli_profile: &Option<String>,
) -> Result<GitBucketWebSession> {
    create_web_session_with_prompt(client, hostname, cli_profile, true).await
}

pub(crate) async fn create_web_session_with_prompt(
    client: &ApiClient,
    hostname: &str,
    cli_profile: &Option<String>,
    allow_prompt: bool,
) -> Result<GitBucketWebSession> {
    let credential = resolve_credential(hostname, cli_profile)?;
    let (username, api_user) = web_actor(client, &credential.host.user).await?;
    let password = match password_from_env() {
        Some(password) => password,
        None => match Key::new(&credential.instance, &username)?.read().await {
            Ok(Some(password)) if !password.is_empty() => password,
            Err(error @ GbError::Cancelled) => return Err(error),
            _ if allow_prompt => prompt_password(&username).await?,
            _ => return Err(GbError::WebPasswordRequired),
        },
    };

    let session = GitBucketWebSession::sign_in(
        &credential.instance,
        &username,
        &password,
        credential.protocol(),
    )
    .await?;
    eprintln!(
        "Notice: Web actor {}; REST actor {}.",
        session.principal(),
        api_user
    );
    Ok(session)
}

pub(crate) async fn save_web_password(credential: &ResolvedCredential) -> Result<()> {
    let client = ApiClient::new(
        &credential.instance,
        &credential.host.token,
        credential.protocol(),
    )?;
    let (username, _) = web_actor(&client, &credential.host.user).await?;
    let password = match password_from_env() {
        Some(password) => password,
        None => prompt_password(&username).await?,
    };
    let session = GitBucketWebSession::sign_in(
        &credential.instance,
        &username,
        &password,
        credential.protocol(),
    )
    .await?;
    Key::new(&credential.instance, session.principal())?
        .save(&password)
        .await?;
    println!(
        "Saved Web password for {} @ {}",
        session.principal(),
        credential.instance
    );
    Ok(())
}

async fn web_actor(client: &ApiClient, saved_user: &str) -> Result<(String, String)> {
    let explicit_web_user = std::env::var("GB_WEB_USER").ok();
    let username = if let Some(user) = &explicit_web_user {
        user.clone()
    } else if let Ok(user) = std::env::var("GB_USER") {
        user
    } else if !saved_user.is_empty() {
        saved_user.to_owned()
    } else {
        String::new()
    };

    if !username.is_empty() {
        crate::target::validate_component(&username)?;
    }
    let api_user = client.current_user().await?;
    let username = if username.is_empty() {
        api_user.login.clone()
    } else {
        username
    };
    crate::target::validate_component(&username)?;
    if explicit_web_user.is_none() && !api_user.login.eq_ignore_ascii_case(&username) {
        return Err(GbError::Auth(format!("REST token belongs to {:?}, but Web username is {:?}. No Web changes were attempted. Set GB_WEB_USER explicitly to authorize a different Web actor.", api_user.login, username)));
    }
    Ok((username, api_user.login.clone()))
}

fn password_from_env() -> Option<String> {
    std::env::var("GB_PASSWORD").ok().filter(|s| !s.is_empty())
}

async fn prompt_password(username: &str) -> Result<String> {
    let prompt = format!("GitBucket password for {username}");
    crate::cancel::prompt(move || Password::new().with_prompt(prompt).interact())
        .await
        .map_err(|error| match error {
            GbError::InputRequired => GbError::WebPasswordRequired,
            error => error,
        })
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
