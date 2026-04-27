use std::collections::BTreeMap;

use clap::{Args, Subcommand};
use dialoguer::{Input, Password};
use serde::Serialize;

use crate::config::auth::{AuthConfig, HostConfig};
use crate::error::{GbError, Result};

#[derive(Args)]
pub struct AuthArgs {
    #[command(subcommand)]
    pub command: AuthCommand,
}

#[derive(Subcommand)]
pub enum AuthCommand {
    /// Authenticate with a GitBucket instance
    Login {
        /// GitBucket host or base URL
        #[arg(long, short = 'H', value_name = "HOST_OR_URL")]
        hostname: Option<String>,
        /// Personal access token (prompts when omitted)
        #[arg(long, short, value_name = "TOKEN")]
        token: Option<String>,
        /// Protocol for bare hosts
        #[arg(long, default_value = "https", value_parser = ["https", "http"])]
        protocol: String,
    },
    /// Remove authentication for a GitBucket instance
    Logout {
        /// Host or base URL to log out from
        #[arg(long, short = 'H', value_name = "HOST_OR_URL")]
        hostname: Option<String>,
    },
    /// Display the authentication status
    Status {
        /// Print status as JSON
        #[arg(long)]
        json: bool,
    },
    /// Print the auth token for a host or URL
    Token {
        /// Host or base URL
        #[arg(long, short = 'H', value_name = "HOST_OR_URL")]
        hostname: Option<String>,
    },
}

pub async fn run(
    args: AuthArgs,
    cli_hostname: &Option<String>,
    cli_profile: &Option<String>,
) -> Result<()> {
    match args.command {
        AuthCommand::Login {
            hostname,
            token,
            protocol,
        } => {
            login(
                hostname.as_ref().or(cli_hostname.as_ref()),
                cli_profile,
                token,
                protocol,
            )
            .await
        }
        AuthCommand::Logout { hostname } => {
            logout(hostname.as_ref().or(cli_hostname.as_ref()), cli_profile).await
        }
        AuthCommand::Status { json } => status(cli_hostname, cli_profile, json).await,
        AuthCommand::Token { hostname } => {
            print_token(hostname.as_ref().or(cli_hostname.as_ref()), cli_profile).await
        }
    }
}

async fn login(
    hostname: Option<&String>,
    cli_profile: &Option<String>,
    token: Option<String>,
    protocol: String,
) -> Result<()> {
    let hostname = match hostname {
        Some(h) => h.clone(),
        None => Input::new()
            .with_prompt(
                "GitBucket host or URL (e.g., gitbucket.example.com or https://gitbucket.example.com/gitbucket)",
            )
            .interact_text()?,
    };

    let token = match token {
        Some(t) => t,
        None => Password::new()
            .with_prompt("Personal access token")
            .interact()?,
    };

    let mut config = AuthConfig::load()?;
    let profile = selected_profile_for_write(&config, cli_profile)?;

    // Verify the token by making a test API call
    let client = crate::api::client::ApiClient::new(&hostname, &token, &protocol)?;
    let user: crate::models::user::User = client
        .get("/user")
        .await
        .map_err(|err| map_login_error(&hostname, err))?;

    config.set_host_for_profile(
        profile.as_deref(),
        hostname.clone(),
        HostConfig {
            token,
            user: user.login.clone(),
            protocol,
        },
    );
    config.save()?;

    println!("✓ Logged in to {} as {}", hostname, user.login);
    Ok(())
}

async fn logout(hostname: Option<&String>, cli_profile: &Option<String>) -> Result<()> {
    let mut config = AuthConfig::load()?;
    if let Some(profile) = selected_profile_for_read(&config, cli_profile)? {
        let hostname = config
            .resolve_hostname(hostname.map(String::as_str), Some(&profile))?
            .ok_or_else(|| crate::error::GbError::Auth("No hosts configured.".into()))?;
        if config.remove_host_for_profile(Some(&profile), &hostname) {
            config.save()?;
            println!("✓ Logged out from {} for profile {}", hostname, profile);
        } else if config.remove_host(&hostname) {
            config.save()?;
            println!(
                "✓ Logged out from {} (global credentials used by profile {})",
                hostname, profile
            );
        } else {
            println!("Not logged in to {} for profile {}", hostname, profile);
        }
        return Ok(());
    }

    let hostname = config
        .resolve_hostname(hostname.map(String::as_str), None)?
        .ok_or_else(|| crate::error::GbError::Auth("No hosts configured.".into()))?;

    if config.remove_host(&hostname) {
        config.save()?;
        println!("✓ Logged out from {}", hostname);
    } else {
        println!("Not logged in to {}", hostname);
    }
    Ok(())
}

#[derive(Serialize)]
struct AuthStatusOutput {
    active_profile: Option<String>,
    effective_actor: Option<EffectiveActorOutput>,
    hosts: BTreeMap<String, HostStatusOutput>,
    profiles: BTreeMap<String, ProfileStatusOutput>,
}

#[derive(Serialize)]
struct ProfileStatusOutput {
    default_host: Option<String>,
    default_repo: Option<String>,
    effective_actor: Option<EffectiveActorOutput>,
    hosts: BTreeMap<String, HostStatusOutput>,
}

#[derive(Serialize, Clone)]
struct EffectiveActorOutput {
    host: String,
    user: String,
    protocol: String,
    credential_source: CredentialSource,
}

#[derive(Serialize)]
struct HostStatusOutput {
    user: String,
    protocol: String,
    has_token: bool,
    credential_source: CredentialSource,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "snake_case")]
enum CredentialSource {
    Profile,
    Global,
    Environment,
}

async fn status(
    cli_hostname: &Option<String>,
    cli_profile: &Option<String>,
    json: bool,
) -> Result<()> {
    let config = AuthConfig::load()?;
    let active_profile = selected_profile_for_read(&config, cli_profile)?;
    let profile_filter = selected_profile_filter(active_profile.as_deref(), cli_profile);
    let output = status_output(
        &config,
        cli_hostname,
        active_profile.as_deref(),
        profile_filter,
    )?;

    if json {
        println!("{}", serde_json::to_string_pretty(&output)?);
        return Ok(());
    }

    if profile_filter.is_some() {
        print_profile_status(&output);
        return Ok(());
    }

    let profile_hosts_empty = config
        .profiles
        .values()
        .all(|profile| profile.hosts.is_empty());
    if config.hosts.is_empty() && profile_hosts_empty {
        println!("Not logged in to any GitBucket instance.");
        println!("Run `gb auth login` to authenticate.");
        return Ok(());
    }

    for (hostname, host_config) in &output.hosts {
        println!("{}", hostname);
        println!("  ✓ Logged in as {}", host_config.user);
        println!("  Protocol: {}", host_config.protocol);
    }
    if !output.profiles.is_empty() {
        println!("Profiles:");
        for (profile_name, profile) in &output.profiles {
            println!("{}", profile_name);
            if let Some(default_host) = &profile.default_host {
                println!("  Default host: {}", default_host);
            }
            if let Some(default_repo) = &profile.default_repo {
                println!("  Default repo: {}", default_repo);
            }
            if let Some(actor) = &profile.effective_actor {
                println!(
                    "  Effective actor: {} @ {} ({})",
                    actor.user,
                    actor.host,
                    credential_source_label(&actor.credential_source)
                );
            }
            for (hostname, host_config) in &profile.hosts {
                println!("  {}", hostname);
                println!("    ✓ Logged in as {}", host_config.user);
                println!("    Protocol: {}", host_config.protocol);
            }
        }
    }
    Ok(())
}

fn status_output(
    config: &AuthConfig,
    cli_hostname: &Option<String>,
    active_profile: Option<&str>,
    profile_filter: Option<&str>,
) -> Result<AuthStatusOutput> {
    let hosts = if profile_filter.is_none() {
        summarize_global_hosts(config)
    } else {
        BTreeMap::new()
    };
    let profiles = match profile_filter {
        Some(profile_name) => {
            let profile = config.profile(profile_name)?;
            BTreeMap::from([(
                profile_name.to_string(),
                profile_status_output(config, profile_name, profile, cli_hostname)?,
            )])
        }
        None => {
            let mut profiles = BTreeMap::new();
            for (profile_name, profile) in &config.profiles {
                profiles.insert(
                    profile_name.clone(),
                    profile_status_output(config, profile_name, profile, &None)?,
                );
            }
            profiles
        }
    };
    let effective_actor = active_profile
        .map(|profile| effective_actor(config, cli_hostname, Some(profile)))
        .transpose()?
        .flatten();

    Ok(AuthStatusOutput {
        active_profile: active_profile.map(str::to_string),
        effective_actor,
        hosts,
        profiles,
    })
}

fn selected_profile_filter<'a>(
    active_profile: Option<&'a str>,
    cli_profile: &Option<String>,
) -> Option<&'a str> {
    if cli_profile.is_some() || std::env::var("GB_PROFILE").is_ok() {
        active_profile
    } else {
        None
    }
}

fn profile_status_output(
    config: &AuthConfig,
    profile_name: &str,
    profile: &crate::config::auth::ProfileConfig,
    cli_hostname: &Option<String>,
) -> Result<ProfileStatusOutput> {
    let effective_actor = effective_actor(config, cli_hostname, Some(profile_name))?;
    let hosts = profile
        .hosts
        .iter()
        .map(|(hostname, host)| {
            (
                hostname.clone(),
                host_status_output(host, CredentialSource::Profile),
            )
        })
        .collect();

    Ok(ProfileStatusOutput {
        default_host: profile.default_host.clone(),
        default_repo: profile.default_repo.clone(),
        effective_actor,
        hosts,
    })
}

fn summarize_global_hosts(config: &AuthConfig) -> BTreeMap<String, HostStatusOutput> {
    config
        .hosts
        .iter()
        .map(|(hostname, host)| {
            (
                hostname.clone(),
                host_status_output(host, CredentialSource::Global),
            )
        })
        .collect()
}

fn host_status_output(host: &HostConfig, credential_source: CredentialSource) -> HostStatusOutput {
    HostStatusOutput {
        user: host.user.clone(),
        protocol: host.protocol.clone(),
        has_token: !host.token.is_empty(),
        credential_source,
    }
}

fn effective_actor(
    config: &AuthConfig,
    cli_hostname: &Option<String>,
    profile: Option<&str>,
) -> Result<Option<EffectiveActorOutput>> {
    let hostname = match config.resolve_hostname(cli_hostname.as_deref(), profile)? {
        Some(hostname) => hostname,
        None => return Ok(None),
    };
    let host = match config.get_host_for_profile(&hostname, profile) {
        Ok(host) => host,
        Err(GbError::NotAuthenticated) => return Ok(None),
        Err(err) => return Err(err),
    };

    let credential_source = if std::env::var("GB_TOKEN").is_ok() {
        CredentialSource::Environment
    } else if profile.is_some_and(|profile| {
        config
            .stored_hostname_for_profile(profile, &hostname)
            .is_some()
    }) {
        CredentialSource::Profile
    } else {
        CredentialSource::Global
    };

    Ok(Some(EffectiveActorOutput {
        host: hostname,
        user: host.user,
        protocol: host.protocol,
        credential_source,
    }))
}

fn print_profile_status(output: &AuthStatusOutput) {
    let Some(profile_name) = output.profiles.keys().next() else {
        return;
    };
    let Some(profile) = output.profiles.get(profile_name) else {
        return;
    };

    println!("Profile: {}", profile_name);
    println!(
        "  Default host: {}",
        profile.default_host.as_deref().unwrap_or("<unset>")
    );
    println!(
        "  Default repo: {}",
        profile.default_repo.as_deref().unwrap_or("<unset>")
    );
    match &profile.effective_actor {
        Some(actor) => println!(
            "  Effective actor: {} @ {} ({})",
            actor.user,
            actor.host,
            credential_source_label(&actor.credential_source)
        ),
        None => println!("  Effective actor: <not authenticated>"),
    }

    if !profile.hosts.is_empty() {
        println!("  Stored credentials:");
        for (hostname, host_config) in &profile.hosts {
            println!("    {}", hostname);
            println!("      ✓ Logged in as {}", host_config.user);
            println!("      Protocol: {}", host_config.protocol);
        }
    }
}

fn credential_source_label(source: &CredentialSource) -> &'static str {
    match source {
        CredentialSource::Profile => "profile credentials",
        CredentialSource::Global => "global credentials",
        CredentialSource::Environment => "environment token",
    }
}

async fn print_token(hostname: Option<&String>, cli_profile: &Option<String>) -> Result<()> {
    let config = AuthConfig::load()?;
    let profile = selected_profile_for_read(&config, cli_profile)?;
    let hostname = config
        .resolve_hostname(hostname.map(String::as_str), profile.as_deref())?
        .ok_or_else(|| crate::error::GbError::Auth("No hosts configured.".into()))?;

    let host = config.get_host_for_profile(&hostname, profile.as_deref())?;
    println!("{}", host.token);
    Ok(())
}

fn selected_profile_for_write(
    config: &AuthConfig,
    cli_profile: &Option<String>,
) -> Result<Option<String>> {
    let profile = cli_profile
        .clone()
        .or_else(|| std::env::var("GB_PROFILE").ok())
        .or_else(|| config.default_profile.clone());

    profile.map(sanitize_profile_name).transpose()
}

fn sanitize_profile_name(profile: String) -> Result<String> {
    let profile = profile.trim();
    if profile.is_empty() {
        Err(GbError::Config("Profile name cannot be empty.".into()))
    } else {
        Ok(profile.to_string())
    }
}

fn selected_profile_for_read(
    config: &AuthConfig,
    cli_profile: &Option<String>,
) -> Result<Option<String>> {
    config.active_profile_name(cli_profile.as_deref())
}

fn map_login_error(hostname: &str, err: GbError) -> GbError {
    match err {
        GbError::Api { status: 404, .. } => GbError::Auth(format!(
            "Failed to authenticate against {} (HTTP 404). The configured host/URL may be missing a GitBucket base path such as `/gitbucket`.",
            hostname
        )),
        GbError::Api { status: 401, .. } => GbError::Auth(format!(
            "Failed to authenticate against {} (HTTP 401). The URL is reachable, but the token was rejected.",
            hostname
        )),
        GbError::Http(source) => GbError::Auth(format!(
            "Failed to connect to {}: {}. Check the protocol, certificate, and GitBucket base URL/path.",
            hostname, source
        )),
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::map_login_error;
    use crate::error::GbError;

    #[test]
    fn maps_404_to_base_path_hint() {
        let err = map_login_error(
            "localhost",
            GbError::Api {
                status: 404,
                message: String::new(),
            },
        );

        match err {
            GbError::Auth(message) => {
                assert!(message.contains("HTTP 404"));
                assert!(message.contains("/gitbucket"));
            }
            other => panic!("expected auth error, got {:?}", other),
        }
    }

    #[test]
    fn maps_401_to_token_hint() {
        let err = map_login_error(
            "https://gitbucket.example.com/gitbucket",
            GbError::Api {
                status: 401,
                message: String::new(),
            },
        );

        match err {
            GbError::Auth(message) => {
                assert!(message.contains("HTTP 401"));
                assert!(message.contains("token was rejected"));
            }
            other => panic!("expected auth error, got {:?}", other),
        }
    }

    #[test]
    fn preserves_unhandled_errors() {
        let err = map_login_error("localhost", GbError::Other("boom".into()));

        match err {
            GbError::Other(message) => assert_eq!(message, "boom"),
            other => panic!("expected original error, got {:?}", other),
        }
    }
}
