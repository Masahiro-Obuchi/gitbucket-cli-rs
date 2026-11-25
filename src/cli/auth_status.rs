//! Saved inventory is distinct from the actor verified for the selected token.
use crate::config::auth::{AuthConfig, CredentialSource, Scope};
use crate::error::{GbError, Result};
use serde::Serialize;

#[derive(Serialize)]
struct AuthStatusOutput {
    active_profile: Option<String>,
    offline: bool,
    effective_actor: Option<Actor>,
}

#[derive(Serialize)]
struct Actor {
    host: String,
    instance: String,
    user: Option<String>,
    stored_user: Option<String>,
    protocol: String,
    credential_source: CredentialSource,
    verification: &'static str,
    error: Option<VerificationError>,
}

#[derive(Serialize)]
struct VerificationError {
    code: &'static str,
    status: Option<u16>,
    cause: Option<&'static str>,
}

pub(crate) async fn print_status(
    config: &AuthConfig,
    cli_hostname: &Option<String>,
    scope: &Scope,
    json: bool,
    offline: bool,
) -> Result<()> {
    let mut actor = None;
    let mut failure = None;
    if let Some(hostname) = config.resolve_hostname_in(cli_hostname.as_deref(), scope)? {
        match config.credential(&hostname, scope) {
            Ok(credential) => {
                let mut selected = Actor {
                    host: hostname,
                    instance: credential.instance.clone(),
                    user: None,
                    stored_user: match credential.source {
                        CredentialSource::Environment => None,
                        _ => Some(credential.host.user.clone()),
                    },
                    protocol: credential.protocol().to_owned(),
                    credential_source: credential.source,
                    verification: "offline",
                    error: None,
                };

                if !offline {
                    let probe = async {
                        let client = crate::api::client::ApiClient::new(
                            &credential.instance,
                            &credential.host.token,
                            credential.protocol(),
                        )?;
                        let user = client.current_user().await?;
                        if user.login.is_empty() {
                            return Err(GbError::Auth(
                                "The authentication probe returned an empty login.".into(),
                            ));
                        }
                        Ok(user.login.clone())
                    }
                    .await;

                    match probe {
                        Ok(user) => {
                            selected.user = Some(user);
                            selected.verification = "verified";
                        }
                        Err(error) => {
                            selected.verification = if matches!(error.status(), Some(401 | 403)) {
                                "rejected"
                            } else {
                                "unverified"
                            };
                            selected.error = Some(VerificationError {
                                code: error.code(),
                                status: error.status(),
                                cause: error.cause_code(),
                            });
                            failure = Some(error);
                        }
                    }
                }
                actor = Some(selected);
            }
            Err(GbError::NotAuthenticated) => (),
            Err(error) => return Err(error),
        }
    }

    let output = AuthStatusOutput {
        active_profile: scope.profile().map(str::to_string),
        offline,
        effective_actor: actor,
    };

    // Status data is also emitted after a failed probe. Preserve that earlier
    // authentication error if writing the status record fails as well.
    let written = if json {
        crate::output::print_json(&output)
    } else {
        print_text(&output);
        Ok(())
    };

    if let Some(error) = failure {
        return Err(error);
    }
    if !offline && output.effective_actor.is_none() {
        return Err(GbError::NotAuthenticated);
    }
    written
}

fn print_text(output: &AuthStatusOutput) {
    if let Some(profile) = &output.active_profile {
        println!("Profile: {profile}");
    }
    if let Some(actor) = &output.effective_actor {
        match &actor.user {
            Some(user) => println!("✓ Logged in as {user} @ {} (verified)", actor.instance),
            None => println!(
                "Actor: <unverified> @ {} ({})",
                actor.instance, actor.verification
            ),
        }
        let source = match actor.credential_source {
            CredentialSource::Environment => "environment token",
            CredentialSource::Profile => "profile credentials",
            CredentialSource::Global => "global credentials",
        };
        println!("Credential source: {source}");
        if let Some(user) = actor
            .stored_user
            .as_ref()
            .filter(|user| Some(*user) != actor.user.as_ref())
        {
            println!("Saved user hint: {user} (not proof of current token ownership)");
        }
    } else {
        println!("No effective credentials for the selected target.");
    }
}
