use clap::{Args, Subcommand, ValueEnum};

use crate::config::{
    auth::{AuthConfig, Scope},
    config_dir,
};
use crate::error::{GbError, Result};

#[derive(Args)]
#[command(
    after_long_help = "Use gb config list for saved entries. See docs/usage/authentication.md for scope and storage."
)]
pub struct ConfigArgs {
    /// Address global settings instead of the selected/default profile
    #[arg(long, global = true)]
    pub global: bool,
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
    /// Get the selected profile's default repository
    DefaultRepo,
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
    /// Clear the selected profile's default repository
    DefaultRepo,
    /// Remove credentials only from the selected scope
    Host {
        #[arg(long = "host", value_name = "HOST_OR_URL")]
        host: String,
    },
    /// Delete a saved profile, clearing default-profile if it points here
    Profile { name: String },
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

pub async fn run(args: ConfigArgs, cli_profile: &Option<String>) -> Result<()> {
    if matches!(&args.command, ConfigCommand::Path) {
        return path();
    }
    let config = AuthConfig::load()?;
    let scope = config.scope(cli_profile.as_deref(), args.global)?;
    match args.command {
        ConfigCommand::Path => path(),
        ConfigCommand::List { json } => list(json),
        ConfigCommand::Get { key } => get(key, &scope),
        ConfigCommand::Set { key } => set(key, &scope),
        ConfigCommand::Unset { key } => unset(key, &scope),
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

fn get(key: ConfigGetKey, scope: &Scope) -> Result<()> {
    let config = AuthConfig::load()?;

    match key {
        ConfigGetKey::DefaultRepo => {
            let name = scope.profile().ok_or_else(|| {
                GbError::Config("default-repo belongs to a profile; select --profile.".into())
            })?;
            println!(
                "{}",
                config
                    .profile(name)?
                    .default_repo
                    .as_deref()
                    .ok_or_else(|| GbError::Config("No default repo configured.".into()))?
            );
        }
        ConfigGetKey::DefaultHost => {
            let hostname = config
                .default_host_in(scope)?
                .map(str::to_string)
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
            let stored_hostname = resolve_saved_hostname(&config, scope, &host)?;
            let stored = &config.hosts_in(scope)?[&stored_hostname];
            crate::cli::config_output::print_host(&stored_hostname, stored, field.as_ref(), json)?;
        }
        ConfigGetKey::Profile { name, json } => {
            let profile = config.profile(&name)?;
            crate::cli::config_output::print_profile(&name, profile, json)?;
        }
    }

    Ok(())
}

fn set(key: ConfigSetKey, scope: &Scope) -> Result<()> {
    let message = AuthConfig::update(|config| {
        let message = match key {
            ConfigSetKey::DefaultHost { hostname } => {
                // Profile defaults can intentionally route to global credentials.
                let instance = match config.saved_key(scope, &hostname)? {
                    Some(key) => key,
                    None if scope.profile().is_some() => {
                        resolve_saved_hostname(config, &Scope::Global, &hostname)?
                    }
                    None => return Err(GbError::Config("No matching saved host entry.".into())),
                };
                *config.default_host_in_mut(scope)? = Some(instance.clone());
                format!("✓ Set default host to {}", instance)
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
                default,
            } => {
                if user.is_none() && !default {
                    return Err(GbError::Config(
                        "Nothing to update. Specify --user or --default.".into(),
                    ));
                }
                let stored = resolve_saved_hostname(config, scope, &host)?;
                let entry = config.hosts_in_mut(scope)?.get_mut(&stored).unwrap();
                if let Some(user) = user {
                    entry.user = user;
                }
                if default {
                    *config.default_host_in_mut(scope)? = Some(stored.clone());
                }
                format!("✓ Updated config for {}", stored)
            }
            ConfigSetKey::Profile {
                name,
                default_host,
                default_repo,
            } => {
                if name.is_empty() || name.trim() != name || name.chars().any(char::is_control) {
                    return Err(GbError::Config("Profile name cannot be empty or contain boundary whitespace/control characters.".into()));
                }
                let default_host = default_host
                    .as_deref()
                    .map(|host| config.profile_default_instance(&name, host))
                    .transpose()?;
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

fn unset(key: ConfigUnsetKey, scope: &Scope) -> Result<()> {
    let message = AuthConfig::update(|config| {
        Ok(match key {
            ConfigUnsetKey::DefaultRepo => {
                let name = scope.profile().ok_or_else(|| {
                    GbError::Config("default-repo belongs to a profile; select --profile.".into())
                })?;
                config.profile(name)?;
                config.profile_mut(name).default_repo = None;
                "✓ Cleared default repo"
            }
            ConfigUnsetKey::Host { host } => {
                if config.remove_in(scope, &host)? {
                    "✓ Removed saved host credentials"
                } else {
                    "No matching host in the selected scope."
                }
            }
            ConfigUnsetKey::Profile { name } => {
                if config.profiles.remove(&name).is_none() {
                    return Err(GbError::Config("No matching saved profile.".into()));
                }
                if config.default_profile.as_deref() == Some(&name) {
                    config.default_profile = None;
                }
                "✓ Removed profile"
            }
            ConfigUnsetKey::DefaultHost => {
                if config.default_host_in_mut(scope)?.take().is_some() {
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

fn resolve_saved_hostname(config: &AuthConfig, scope: &Scope, host: &str) -> Result<String> {
    config.saved_key(scope, host)?.ok_or_else(|| {
        GbError::Config(format!(
            "No saved host entry matches {}. Run `gb auth login -H {}` first.",
            host, host
        ))
    })
}
