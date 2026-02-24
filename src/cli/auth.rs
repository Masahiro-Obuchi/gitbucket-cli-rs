use clap::{Args, Subcommand};
use dialoguer::{Input, Password};

use crate::config::auth::{AuthConfig, HostConfig};
use crate::error::Result;

#[derive(Args)]
pub struct AuthArgs {
    #[command(subcommand)]
    pub command: AuthCommand,
}

#[derive(Subcommand)]
pub enum AuthCommand {
    /// Authenticate with a GitBucket instance
    Login {
        /// GitBucket hostname (e.g., gitbucket.example.com)
        #[arg(long, short = 'H')]
        hostname: Option<String>,
        /// Personal access token
        #[arg(long, short)]
        token: Option<String>,
        /// Protocol (https or http)
        #[arg(long, default_value = "https")]
        protocol: String,
    },
    /// Remove authentication for a GitBucket instance
    Logout {
        /// Hostname to logout from
        #[arg(long, short = 'H')]
        hostname: Option<String>,
    },
    /// Display the authentication status
    Status,
    /// Print the auth token for a hostname
    Token {
        /// Hostname
        #[arg(long, short = 'H')]
        hostname: Option<String>,
    },
}

pub async fn run(args: AuthArgs, cli_hostname: &Option<String>) -> Result<()> {
    match args.command {
        AuthCommand::Login {
            hostname,
            token,
            protocol,
        } => login(hostname.as_ref().or(cli_hostname.as_ref()), token, protocol).await,
        AuthCommand::Logout { hostname } => {
            logout(hostname.as_ref().or(cli_hostname.as_ref())).await
        }
        AuthCommand::Status => status().await,
        AuthCommand::Token { hostname } => {
            print_token(hostname.as_ref().or(cli_hostname.as_ref())).await
        }
    }
}

async fn login(
    hostname: Option<&String>,
    token: Option<String>,
    protocol: String,
) -> Result<()> {
    let hostname = match hostname {
        Some(h) => h.clone(),
        None => Input::new()
            .with_prompt("GitBucket hostname (e.g., gitbucket.example.com)")
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
    let user: crate::models::user::User = client.get("/user").await.map_err(|_| {
        crate::error::GbError::Auth("Failed to authenticate. Check your token and hostname.".into())
    })?;

    let mut config = AuthConfig::load()?;
    config.set_host(
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

async fn logout(hostname: Option<&String>) -> Result<()> {
    let mut config = AuthConfig::load()?;
    let hostname = match hostname {
        Some(h) => h.clone(),
        None => config
            .default_hostname()
            .ok_or_else(|| crate::error::GbError::Auth("No hosts configured.".into()))?,
    };

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
    if config.hosts.is_empty() {
        println!("Not logged in to any GitBucket instance.");
        println!("Run `gb auth login` to authenticate.");
        return Ok(());
    }

    for (hostname, host_config) in &config.hosts {
        println!("{}", hostname);
        println!("  ✓ Logged in as {}", host_config.user);
        println!("  Protocol: {}", host_config.protocol);
    }
    Ok(())
}

async fn print_token(hostname: Option<&String>) -> Result<()> {
    let config = AuthConfig::load()?;
    let hostname = match hostname {
        Some(h) => h.clone(),
        None => config.default_hostname().ok_or_else(|| {
            crate::error::GbError::Auth("No hosts configured.".into())
        })?,
    };

    let host = config.get_host(&hostname)?;
    println!("{}", host.token);
    Ok(())
}
