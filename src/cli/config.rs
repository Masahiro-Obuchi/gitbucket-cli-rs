use clap::{Args, Subcommand, ValueEnum};

use crate::config::{auth::AuthConfig, config_dir};
use crate::error::{GbError, Result};

#[derive(Args)]
#[command(
    after_help = "Updates atomically replace config.toml and wait up to 5 seconds for an OS lock.\nThe empty config.toml.lock file remains after exit; do not delete it.\nConfiguration and lock files must be regular files (no symlinks/reparse points)."
)]
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
    /// Get the stored default profile
    DefaultProfile,
    /// Get configuration for a saved host
    Host {
        /// Saved host or base URL to inspect
        #[arg(long = "host", value_name = "HOST_OR_URL")]
        host: String,
        /// Specific field to print
        #[arg(long)]
        field: Option<HostField>,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
    /// Get configuration for a saved profile
    Profile {
        /// Profile name to inspect
        name: String,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand)]
pub enum ConfigSetKey {
    /// Set the stored default host
    DefaultHost {
        /// Saved host or base URL to make default
        #[arg(value_name = "HOST_OR_URL")]
        hostname: String,
    },
    /// Set the stored default profile
    DefaultProfile {
        /// Saved profile name to make default
        name: String,
    },
    /// Update fields for an existing saved host
    Host {
        /// Saved host or base URL to update
        #[arg(long = "host", value_name = "HOST_OR_URL")]
        host: String,
        /// Username to store for web-session fallbacks
        #[arg(long)]
        user: Option<String>,
        /// Protocol to store for this host
        #[arg(long, value_parser = ["https", "http"])]
        protocol: Option<String>,
        /// Also make this host the stored default
        #[arg(long)]
        default: bool,
    },
    /// Create or update a profile
    Profile {
        /// Profile name to create or update
        name: String,
        /// Default host or base URL for this profile
        #[arg(long = "default-host", value_name = "HOST_OR_URL")]
        default_host: Option<String>,
        /// Default repository for this profile
        #[arg(long = "default-repo", value_name = "OWNER/REPO")]
        default_repo: Option<String>,
    },
}

#[derive(Subcommand)]
pub enum ConfigUnsetKey {
    /// Clear the stored default host
    DefaultHost,
    /// Clear the stored default profile
    DefaultProfile,
}

#[derive(Clone, Debug, ValueEnum)]
pub enum HostField {
    User,
    Protocol,
    HasToken,
}

pub async fn run(args: ConfigArgs, _cli_profile: &Option<String>) -> Result<()> {
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

    crate::cli::config_output::print_list(&config, &path, json)
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
        ConfigGetKey::DefaultProfile => {
            let profile = config
                .default_profile
                .ok_or_else(|| GbError::Config("No default profile configured.".into()))?;
            println!("{}", profile);
        }
        ConfigGetKey::Host { host, field, json } => {
            let stored_hostname = resolve_saved_hostname(&config, &host)?;
            let stored = config.hosts.get(&stored_hostname).unwrap();
            crate::cli::config_output::print_host(&stored_hostname, stored, field.as_ref(), json)?;
        }
        ConfigGetKey::Profile { name, json } => {
            let profile = config.profile(&name)?;
            crate::cli::config_output::print_profile(&name, profile, json)?;
        }
    }

    Ok(())
}

fn set(key: ConfigSetKey) -> Result<()> {
    let message = AuthConfig::update(|config| {
        let message = match key {
            ConfigSetKey::DefaultHost { hostname } => {
                let stored = resolve_saved_hostname(config, &hostname)?;
                config.default_host = Some(stored.clone());
                format!("✓ Set default host to {}", stored)
            }
            ConfigSetKey::DefaultProfile { name } => {
                if !config.profiles.contains_key(&name) {
                    return Err(GbError::Config(format!(
                        "No saved profile named {}. Run `gb config set profile {}` first.",
                        name, name
                    )));
                }
                config.default_profile = Some(name.clone());
                format!("✓ Set default profile to {}", name)
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
                let stored = resolve_saved_hostname(config, &host)?;
                let entry = config.hosts.get_mut(&stored).unwrap();
                if let Some(user) = user {
                    entry.user = user;
                }
                if let Some(protocol) = protocol {
                    validate_protocol(&protocol)?;
                    entry.protocol = protocol;
                }
                if default {
                    config.default_host = Some(stored.clone());
                }
                format!("✓ Updated config for {}", stored)
            }
            ConfigSetKey::Profile {
                name,
                default_host,
                default_repo,
            } => {
                if let Some(repo) = default_repo.as_deref() {
                    crate::cli::common::parse_owner_repo(repo)?;
                }
                let profile = config.profile_mut(&name);
                if let Some(host) = default_host {
                    profile.default_host = Some(host);
                }
                if let Some(repo) = default_repo {
                    profile.default_repo = Some(repo);
                }
                format!("✓ Updated profile {}", name)
            }
        };
        Ok(message)
    })?;
    println!("{}", message);
    Ok(())
}

fn unset(key: ConfigUnsetKey) -> Result<()> {
    let message = AuthConfig::update(|config| {
        Ok(match key {
            ConfigUnsetKey::DefaultHost => {
                if config.default_host.take().is_some() {
                    "✓ Cleared default host"
                } else {
                    "No default host configured."
                }
            }
            ConfigUnsetKey::DefaultProfile => {
                if config.default_profile.take().is_some() {
                    "✓ Cleared default profile"
                } else {
                    "No default profile configured."
                }
            }
        })
    })?;
    println!("{}", message);
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
