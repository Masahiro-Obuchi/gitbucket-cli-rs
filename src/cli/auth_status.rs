//! Saved inventory is distinct from the actor verified for the selected token.
use crate::config::auth::{AuthConfig, CredentialSource, HostConfig, Scope};
use crate::error::{GbError, Result};
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Serialize)]
struct AuthStatusOutput {
    active_profile: Option<String>,
    offline: bool,
    effective_actor: Option<Actor>,
    hosts: BTreeMap<String, SavedHost>,
    profiles: BTreeMap<String, SavedProfile>,
}
#[derive(Clone, Serialize)]
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
#[derive(Clone, Serialize)]
struct VerificationError {
    code: &'static str,
    status: Option<u16>,
    cause: Option<&'static str>,
}
#[derive(Serialize)]
struct SavedHost {
    user: String,
    protocol: String,
    has_token: bool,
    credential_source: CredentialSource,
    verification: &'static str,
}
#[derive(Serialize)]
struct SavedProfile {
    default_host: Option<String>,
    default_repo: Option<String>,
    effective_actor: Option<Actor>,
    hosts: BTreeMap<String, SavedHost>,
}

fn inventory(
    hosts: &std::collections::HashMap<String, HostConfig>,
    source: CredentialSource,
) -> BTreeMap<String, SavedHost> {
    hosts
        .iter()
        .map(|(key, host)| {
            (
                key.clone(),
                SavedHost {
                    user: host.user.clone(),
                    protocol: host.protocol.clone(),
                    has_token: !host.token.is_empty(),
                    credential_source: source,
                    verification: "not_checked",
                },
            )
        })
        .collect()
}

pub(crate) async fn print_status(
    config: &AuthConfig,
    cli_hostname: &Option<String>,
    scope: &Scope,
    filter_scope: bool,
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
                    protocol: credential.host.protocol.clone(),
                    credential_source: credential.source,
                    verification: "offline",
                    error: None,
                };
                if !offline {
                    let probe = async {
                        let client = crate::api::client::ApiClient::new(
                            &credential.instance,
                            &credential.host.token,
                            &credential.host.protocol,
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
    let mut profiles = BTreeMap::new();
    if !filter_scope || scope.profile().is_some() {
        for (name, profile) in &config.profiles {
            if filter_scope && scope.profile() != Some(name.as_str()) {
                continue;
            }
            profiles.insert(
                name.clone(),
                SavedProfile {
                    default_host: profile.default_host.clone(),
                    default_repo: profile.default_repo.clone(),
                    effective_actor: if scope.profile() == Some(name.as_str()) {
                        actor.clone()
                    } else {
                        None
                    },
                    hosts: inventory(&profile.hosts, CredentialSource::Profile),
                },
            );
        }
    }
    let output = AuthStatusOutput {
        active_profile: scope.profile().map(str::to_string),
        offline,
        effective_actor: actor,
        hosts: if !filter_scope || scope.profile().is_none() {
            inventory(&config.hosts, CredentialSource::Global)
        } else {
            BTreeMap::new()
        },
        profiles,
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
        if let Some(user) = &actor.stored_user {
            println!("Saved user hint: {user} (not proof of current token ownership)");
        }
    } else {
        println!("No effective credentials for the selected target.");
    }
    for (key, host) in &output.hosts {
        println!("{key}: saved user {} (unverified)", host.user);
    }
    for (name, profile) in &output.profiles {
        println!("Saved profile: {name}");
        if let Some(repo) = &profile.default_repo {
            println!("  Default repo: {repo}");
        }
        for (key, host) in &profile.hosts {
            println!("  {key}: saved user {} (unverified)", host.user);
        }
    }
}
