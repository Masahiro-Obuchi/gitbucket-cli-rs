use clap::{Args, Subcommand};
use dialoguer::{Input, Password};

use crate::config::auth::{instance_key, instance_protocol, AuthConfig, HostConfig, Scope};
use crate::config::web_password::Key;
use crate::error::{GbError, Result};

#[derive(Args)]
#[command(after_long_help = "--global selects the global scope. See docs/usage/authentication.md.")]
pub struct AuthArgs {
    /// Use global credentials, bypassing the saved default profile
    #[arg(long, global = true)]
    pub global: bool,
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
        /// Read a single personal access token from stdin
        #[arg(long)]
        with_token: bool,
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
    /// Show the selected instance and token verification
    #[command(
        after_long_help = "Verifies the selected token unless --offline is set. Use gb config list for saved entries."
    )]
    Status {
        /// Show the selected credentials without verifying them
        #[arg(long)]
        offline: bool,
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
    /// Verify and save a Web password in the OS credential store
    WebLogin,
    /// Remove a Web password from the OS credential store
    WebLogout {
        /// Web username of the credential to remove
        #[arg(long)]
        user: String,
    },
}

pub async fn run(
    args: AuthArgs,
    cli_hostname: &Option<String>,
    cli_profile: &Option<String>,
    diagnostic_token: &mut Option<String>,
) -> Result<()> {
    let config = AuthConfig::load()?;
    let scope = config.scope(cli_profile.as_deref(), args.global)?;
    match args.command {
        AuthCommand::Login {
            hostname,
            with_token,
            protocol,
        } => {
            login(
                hostname.as_ref().or(cli_hostname.as_ref()),
                &scope,
                with_token,
                protocol,
                diagnostic_token,
            )
            .await
        }
        AuthCommand::Logout { hostname } => {
            logout(hostname.as_ref().or(cli_hostname.as_ref()), &scope).await
        }
        AuthCommand::Status { json, offline } => {
            crate::cli::auth_status::print_status(&config, cli_hostname, &scope, json, offline)
                .await
        }
        AuthCommand::Token { hostname } => {
            print_token(hostname.as_ref().or(cli_hostname.as_ref()), &scope).await
        }
        AuthCommand::WebLogin => {
            let hostname = web_hostname(&config, cli_hostname, &scope)?;
            let credential = config.credential(&hostname, &scope)?;
            super::common::save_web_password(&credential).await
        }
        AuthCommand::WebLogout { user } => {
            let hostname = web_hostname(&config, cli_hostname, &scope)?;
            if !hostname.contains("://") {
                return Err(GbError::Config(
                    "Use the full GitBucket base URL with --hostname to remove a Web password."
                        .into(),
                ));
            }
            let key = Key::new(&hostname, &user)?;
            let instance = key.instance();
            if key.delete().await? {
                println!("Removed Web password for {user} @ {instance}");
            } else {
                println!("No Web password saved for {user} @ {instance}");
            }
            Ok(())
        }
    }
}

fn web_hostname(config: &AuthConfig, hostname: &Option<String>, scope: &Scope) -> Result<String> {
    config
        .resolve_hostname_in(hostname.as_deref(), scope)?
        .ok_or_else(|| GbError::Config("Specify the GitBucket base URL with --hostname.".into()))
}

async fn login(
    hostname: Option<&String>,
    scope: &Scope,
    with_token: bool,
    protocol: String,
    diagnostic_token: &mut Option<String>,
) -> Result<()> {
    let hostname = match hostname {
        Some(h) => h.clone(),
        None if !super::input::interactive() => return Err(super::input::required("--hostname")),
        None => crate::cancel::prompt(|| {
            Input::new()
            .with_prompt(
                "GitBucket host or URL (e.g., gitbucket.example.com or https://gitbucket.example.com/gitbucket)",
            )
            .interact_text()
        }).await?,
    };

    let token = if with_token {
        super::input::token_stdin().await?
    } else if super::input::interactive() {
        crate::cancel::prompt(|| {
            Password::new()
                .with_prompt("Personal access token")
                .interact()
        })
        .await?
    } else {
        return Err(super::input::required("--with-token"));
    };

    // Final diagnostics can include context beyond the transport's redaction.
    *diagnostic_token = Some(token.clone());
    let instance = instance_key(&hostname, &protocol)?;
    let client =
        crate::api::client::ApiClient::new(&instance, &token, instance_protocol(&instance))?;
    let user: crate::models::user::User = client
        .get("/user")
        .await
        .map_err(|err| map_login_error(&hostname, err))?;

    AuthConfig::update(|config| {
        config.save_login(
            scope,
            instance,
            HostConfig {
                token,
                user: user.login.clone(),
            },
        );
        Ok(())
    })?;

    println!("✓ Logged in to {} as {}", hostname, user.login);
    Ok(())
}

async fn logout(hostname: Option<&String>, scope: &Scope) -> Result<()> {
    let message = AuthConfig::update(|config| {
        if let Some(profile) = scope.profile() {
            let hostname = config
                .resolve_hostname_in(hostname.map(String::as_str), scope)?
                .ok_or_else(|| GbError::Auth("No hosts configured.".into()))?;
            if config.remove_in(scope, &hostname)? {
                return Ok(format!(
                    "✓ Logged out from {} for profile {}",
                    hostname, profile
                ));
            }
            return Ok(format!(
                "No saved credentials for {} in profile {}; global fallback credentials were retained",
                hostname, profile
            ));
        }
        let hostname = config
            .resolve_hostname_in(hostname.map(String::as_str), scope)?
            .ok_or_else(|| GbError::Auth("No hosts configured.".into()))?;
        Ok(if config.remove_in(scope, &hostname)? {
            format!("✓ Logged out from {}", hostname)
        } else {
            format!("Not logged in to {}", hostname)
        })
    })?;
    println!("{}", message);
    if std::env::var_os("GB_TOKEN").is_some() {
        println!("GB_TOKEN remains active; logout does not modify environment variables.");
    }
    if scope.profile().is_some() {
        println!("Global entries are unchanged and may still supply fallback credentials. Use `gb auth logout --global -H <URL>` to remove them explicitly.");
    }
    Ok(())
}

async fn print_token(hostname: Option<&String>, scope: &Scope) -> Result<()> {
    let config = AuthConfig::load()?;
    let hostname = config
        .resolve_hostname_in(hostname.map(String::as_str), scope)?
        .ok_or_else(|| crate::error::GbError::Auth("No hosts configured.".into()))?;

    let credential = config.credential(&hostname, scope)?;
    println!("{}", credential.host.token);
    Ok(())
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
