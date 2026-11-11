use clap::{Args, Subcommand};
use dialoguer::{Input, Password};

use crate::config::auth::{instance_key, AuthConfig, HostConfig, Scope};
use crate::error::{GbError, Result};

#[derive(Args)]
#[command(
    after_help = "Storage scope follows --profile, GB_PROFILE, then default_profile. --global bypasses default_profile and conflicts with an explicit --profile/GB_PROFILE. Logout removes only the selected scope; environment tokens and global fallback may remain active."
)]
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
    /// Verify the selected token and show saved credentials separately
    #[command(
        after_help = "By default, verify the selected token with GET /user. Only a verified response supplies effective_actor.user. --offline makes no request and leaves the actor unknown. Rejected or unverifiable online credentials exit 1; JSON status is still printed without tokens. Other saved entries are inventory and are not probed."
    )]
    Status {
        /// Show saved state without contacting the server; actor remains unverified
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
}

pub async fn run(
    args: AuthArgs,
    cli_hostname: &Option<String>,
    cli_profile: &Option<String>,
) -> Result<()> {
    let config = AuthConfig::load()?;
    let scope = config.scope(cli_profile.as_deref(), args.global)?;
    match args.command {
        AuthCommand::Login {
            hostname,
            token,
            protocol,
        } => {
            login(
                hostname.as_ref().or(cli_hostname.as_ref()),
                &scope,
                token,
                protocol,
            )
            .await
        }
        AuthCommand::Logout { hostname } => {
            logout(hostname.as_ref().or(cli_hostname.as_ref()), &scope).await
        }
        AuthCommand::Status { json, offline } => {
            crate::cli::auth_status::print_status(
                &config,
                cli_hostname,
                &scope,
                args.global || cli_profile.is_some() || std::env::var_os("GB_PROFILE").is_some(),
                json,
                offline,
            )
            .await
        }
        AuthCommand::Token { hostname } => {
            print_token(hostname.as_ref().or(cli_hostname.as_ref()), &scope).await
        }
    }
}

async fn login(
    hostname: Option<&String>,
    scope: &Scope,
    token: Option<String>,
    protocol: String,
) -> Result<()> {
    let hostname = match hostname {
        Some(h) => h.clone(),
        None => crate::cancel::input(|| {
            Input::new()
            .with_prompt(
                "GitBucket host or URL (e.g., gitbucket.example.com or https://gitbucket.example.com/gitbucket)",
            )
            .interact_text()
        }).await?,
    };

    let token = match token {
        Some(t) => t,
        None => {
            crate::cancel::input(|| {
                Password::new()
                    .with_prompt("Personal access token")
                    .interact()
            })
            .await?
        }
    };

    let instance = instance_key(&hostname, &protocol)?;
    let protocol = url::Url::parse(&instance)?.scheme().to_string();

    // Verify the token by making a test API call
    let client = crate::api::client::ApiClient::new(&hostname, &token, &protocol)?;
    let user: crate::models::user::User = client
        .get("/user")
        .await
        .map_err(|err| map_login_error(&hostname, err))?;

    AuthConfig::update(|config| {
        config.save_login(
            scope,
            &instance,
            HostConfig {
                token,
                user: user.login.clone(),
                protocol,
            },
        )
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
        Ok(if config.remove_host(&hostname)? {
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
