use clap::{Args, Subcommand};
use dialoguer::{Input, Password};

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
    Status,
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
        AuthCommand::Status => status().await,
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

    // Verify the token by making a test API call
    let client = crate::api::client::ApiClient::new(&hostname, &token, &protocol)?;
    let user: crate::models::user::User = client
        .get("/user")
        .await
        .map_err(|err| map_login_error(&hostname, err))?;

    let mut config = AuthConfig::load()?;
    let profile = selected_profile_for_write(&config, cli_profile);
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

async fn status() -> Result<()> {
    let config = AuthConfig::load()?;
    let profile_hosts_empty = config
        .profiles
        .values()
        .all(|profile| profile.hosts.is_empty());
    if config.hosts.is_empty() && profile_hosts_empty {
        println!("Not logged in to any GitBucket instance.");
        println!("Run `gb auth login` to authenticate.");
        return Ok(());
    }

    for (hostname, host_config) in &config.hosts {
        println!("{}", hostname);
        println!("  ✓ Logged in as {}", host_config.user);
        println!("  Protocol: {}", host_config.protocol);
    }
    if !config.profiles.is_empty() {
        println!("Profiles:");
        for (profile_name, profile) in &config.profiles {
            println!("{}", profile_name);
            if let Some(default_host) = &profile.default_host {
                println!("  Default host: {}", default_host);
            }
            if let Some(default_repo) = &profile.default_repo {
                println!("  Default repo: {}", default_repo);
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

fn selected_profile_for_write(config: &AuthConfig, cli_profile: &Option<String>) -> Option<String> {
    cli_profile
        .clone()
        .or_else(|| std::env::var("GB_PROFILE").ok())
        .or_else(|| config.default_profile.clone())
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
