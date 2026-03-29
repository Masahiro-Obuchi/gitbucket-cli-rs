use std::collections::BTreeMap;

use clap::{Args, Subcommand, ValueEnum};
use serde::Serialize;

use crate::config::{auth::AuthConfig, config_dir};
use crate::error::{GbError, Result};

#[derive(Args)]
pub struct ConfigArgs {
    #[command(subcommand)]
    pub command: ConfigCommand,
}

#[derive(Subcommand)]
pub enum ConfigCommand {
    /// Print the config file path
    Path,
    /// List stored configuration values
    List {
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
    /// Read a configuration value
    Get {
        #[command(subcommand)]
        key: ConfigGetKey,
    },
    /// Update a configuration value
    Set {
        #[command(subcommand)]
        key: ConfigSetKey,
    },
    /// Clear a configuration value
    Unset {
        #[command(subcommand)]
        key: ConfigUnsetKey,
    },
}

#[derive(Subcommand)]
pub enum ConfigGetKey {
    /// Get the stored default host
    DefaultHost,
    /// Get configuration for a saved host
    Host {
        /// Host or base URL to inspect
        #[arg(long = "host")]
        host: String,
        /// Specific field to print
        #[arg(long)]
        field: Option<HostField>,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand)]
pub enum ConfigSetKey {
    /// Set the stored default host
    DefaultHost {
        /// Host or base URL to make default
        hostname: String,
    },
    /// Update fields for an existing saved host
    Host {
        /// Host or base URL to update
        #[arg(long = "host")]
        host: String,
        /// Username to store for web-session fallbacks
        #[arg(long)]
        user: Option<String>,
        /// Protocol to store for this host
        #[arg(long)]
        protocol: Option<String>,
        /// Also make this host the stored default
        #[arg(long)]
        default: bool,
    },
}

#[derive(Subcommand)]
pub enum ConfigUnsetKey {
    /// Clear the stored default host
    DefaultHost,
}

#[derive(Clone, Debug, ValueEnum)]
pub enum HostField {
    User,
    Protocol,
    HasToken,
}

#[derive(Serialize)]
struct ConfigListOutput {
    path: String,
    default_host: Option<String>,
    hosts: BTreeMap<String, HostSummary>,
}

#[derive(Serialize)]
struct HostOutput {
    hostname: String,
    user: String,
    protocol: String,
    has_token: bool,
}

#[derive(Serialize)]
struct HostSummary {
    user: String,
    protocol: String,
    has_token: bool,
}

pub async fn run(args: ConfigArgs) -> Result<()> {
    match args.command {
        ConfigCommand::Path => path(),
        ConfigCommand::List { json } => list(json),
        ConfigCommand::Get { key } => get(key),
        ConfigCommand::Set { key } => set(key),
        ConfigCommand::Unset { key } => unset(key),
    }
}

fn path() -> Result<()> {
    println!("{}", config_dir()?.join("config.toml").display());
    Ok(())
}

fn list(json: bool) -> Result<()> {
    let config = AuthConfig::load()?;
    let path = config_dir()?.join("config.toml");

    let hosts: BTreeMap<String, HostSummary> = config
        .hosts
        .iter()
        .map(|(hostname, host)| {
            (
                hostname.clone(),
                HostSummary {
                    user: host.user.clone(),
                    protocol: host.protocol.clone(),
                    has_token: !host.token.is_empty(),
                },
            )
        })
        .collect();

    if json {
        let payload = ConfigListOutput {
            path: path.display().to_string(),
            default_host: config.default_host.clone(),
            hosts,
        };
        println!("{}", serde_json::to_string_pretty(&payload)?);
        return Ok(());
    }

    println!("Path: {}", path.display());
    match &config.default_host {
        Some(hostname) => {
            if config.hosts.contains_key(hostname) {
                println!("Default host: {}", hostname);
            } else {
                println!("Default host: {} (missing saved host entry)", hostname);
            }
        }
        None => println!("Default host: <unset>"),
    }

    if hosts.is_empty() {
        println!("Hosts: <none>");
        return Ok(());
    }

    println!("Hosts:");
    for (hostname, host) in hosts {
        println!("{}", hostname);
        println!("  User: {}", display_user(&host.user));
        println!("  Protocol: {}", host.protocol);
        println!(
            "  Token: {}",
            if host.has_token {
                "configured"
            } else {
                "missing"
            }
        );
    }

    Ok(())
}

fn get(key: ConfigGetKey) -> Result<()> {
    let config = AuthConfig::load()?;

    match key {
        ConfigGetKey::DefaultHost => {
            let hostname = config
                .default_host
                .ok_or_else(|| GbError::Config("No default host configured.".into()))?;
            println!("{}", hostname);
        }
        ConfigGetKey::Host { host, field, json } => {
            let stored_hostname = resolve_saved_hostname(&config, &host)?;
            let stored = config.hosts.get(&stored_hostname).unwrap();
            let output = HostOutput {
                hostname: stored_hostname.clone(),
                user: stored.user.clone(),
                protocol: stored.protocol.clone(),
                has_token: !stored.token.is_empty(),
            };

            if json {
                println!("{}", serde_json::to_string_pretty(&output)?);
                return Ok(());
            }

            match field {
                Some(HostField::User) => println!("{}", output.user),
                Some(HostField::Protocol) => println!("{}", output.protocol),
                Some(HostField::HasToken) => println!("{}", output.has_token),
                None => {
                    println!("Host: {}", output.hostname);
                    println!("User: {}", display_user(&output.user));
                    println!("Protocol: {}", output.protocol);
                    println!(
                        "Token: {}",
                        if output.has_token {
                            "configured"
                        } else {
                            "missing"
                        }
                    );
                }
            }
        }
    }

    Ok(())
}

fn set(key: ConfigSetKey) -> Result<()> {
    let mut config = AuthConfig::load()?;

    match key {
        ConfigSetKey::DefaultHost { hostname } => {
            let stored_hostname = resolve_saved_hostname(&config, &hostname)?;
            config.default_host = Some(stored_hostname.clone());
            config.save()?;
            println!("✓ Set default host to {}", stored_hostname);
        }
        ConfigSetKey::Host {
            host,
            user,
            protocol,
            default,
        } => {
            if user.is_none() && protocol.is_none() && !default {
                return Err(GbError::Config(
                    "Nothing to update. Specify --user, --protocol, or --default.".into(),
                ));
            }

            let stored_hostname = resolve_saved_hostname(&config, &host)?;
            let entry = config.hosts.get_mut(&stored_hostname).unwrap();

            if let Some(user) = user {
                entry.user = user;
            }

            if let Some(protocol) = protocol {
                validate_protocol(&protocol)?;
                entry.protocol = protocol;
            }

            if default {
                config.default_host = Some(stored_hostname.clone());
            }

            config.save()?;
            println!("✓ Updated config for {}", stored_hostname);
        }
    }

    Ok(())
}

fn unset(key: ConfigUnsetKey) -> Result<()> {
    let mut config = AuthConfig::load()?;

    match key {
        ConfigUnsetKey::DefaultHost => {
            if config.default_host.is_none() {
                println!("No default host configured.");
                return Ok(());
            }
            config.default_host = None;
            config.save()?;
            println!("✓ Cleared default host");
        }
    }

    Ok(())
}

fn resolve_saved_hostname(config: &AuthConfig, host: &str) -> Result<String> {
    config.stored_hostname(host).ok_or_else(|| {
        GbError::Config(format!(
            "No saved host entry matches {}. Run `gb auth login -H {}` first.",
            host, host
        ))
    })
}

fn validate_protocol(protocol: &str) -> Result<()> {
    match protocol {
        "http" | "https" => Ok(()),
        _ => Err(GbError::Config(
            "Protocol must be `http` or `https`.".into(),
        )),
    }
}

fn display_user(user: &str) -> &str {
    if user.is_empty() {
        "<unset>"
    } else {
        user
    }
}
