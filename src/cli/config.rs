use std::collections::{BTreeMap, HashMap};

use clap::{Args, Subcommand, ValueEnum};
use serde::Serialize;

use crate::config::{
    auth::{AuthConfig, HostConfig, ProfileConfig},
    config_dir,
};
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

#[derive(Serialize)]
struct ConfigListOutput {
    path: String,
    default_host: Option<String>,
    default_profile: Option<String>,
    hosts: BTreeMap<String, HostSummary>,
    profiles: BTreeMap<String, ProfileOutput>,
}

#[derive(Serialize)]
struct HostOutput {
    hostname: String,
    user: String,
    protocol: String,
    has_token: bool,
}

#[derive(Serialize)]
struct ProfileOutput {
    name: String,
    default_host: Option<String>,
    default_repo: Option<String>,
    hosts: BTreeMap<String, HostSummary>,
}

#[derive(Serialize)]
struct HostSummary {
    user: String,
    protocol: String,
    has_token: bool,
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

    let hosts = summarize_hosts(&config.hosts);
    let profiles = summarize_profiles(&config.profiles);

    if json {
        let payload = ConfigListOutput {
            path: path.display().to_string(),
            default_host: config.default_host.clone(),
            default_profile: config.default_profile.clone(),
            hosts,
            profiles,
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
    match &config.default_profile {
        Some(profile) => {
            if config.profiles.contains_key(profile) {
                println!("Default profile: {}", profile);
            } else {
                println!("Default profile: {} (missing saved profile entry)", profile);
            }
        }
        None => println!("Default profile: <unset>"),
    }

    if hosts.is_empty() {
        println!("Hosts: <none>");
    } else {
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
    }

    if profiles.is_empty() {
        println!("Profiles: <none>");
    } else {
        println!("Profiles:");
        for (name, profile) in profiles {
            println!("{}", name);
            println!(
                "  Default host: {}",
                profile.default_host.as_deref().unwrap_or("<unset>")
            );
            println!(
                "  Default repo: {}",
                profile.default_repo.as_deref().unwrap_or("<unset>")
            );
            if profile.hosts.is_empty() {
                println!("  Hosts: <none>");
            } else {
                println!("  Hosts:");
                for (hostname, host) in profile.hosts {
                    println!("  {}", hostname);
                    println!("    User: {}", display_user(&host.user));
                    println!("    Protocol: {}", host.protocol);
                    println!(
                        "    Token: {}",
                        if host.has_token {
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
        ConfigGetKey::Profile { name, json } => {
            let profile = config.profile(&name)?;
            let output = profile_output(&name, profile);

            if json {
                println!("{}", serde_json::to_string_pretty(&output)?);
                return Ok(());
            }

            println!("Profile: {}", output.name);
            println!(
                "Default host: {}",
                output.default_host.as_deref().unwrap_or("<unset>")
            );
            println!(
                "Default repo: {}",
                output.default_repo.as_deref().unwrap_or("<unset>")
            );
            if output.hosts.is_empty() {
                println!("Hosts: <none>");
            } else {
                println!("Hosts:");
                for (hostname, host) in output.hosts {
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
        ConfigSetKey::DefaultProfile { name } => {
            if !config.profiles.contains_key(&name) {
                return Err(GbError::Config(format!(
                    "No saved profile named {}. Run `gb config set profile {}` first.",
                    name, name
                )));
            }
            config.default_profile = Some(name.clone());
            config.save()?;
            println!("✓ Set default profile to {}", name);
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
        ConfigSetKey::Profile {
            name,
            default_host,
            default_repo,
        } => {
            if let Some(repo) = default_repo.as_deref() {
                crate::cli::common::parse_owner_repo(repo)?;
            }

            let profile = config.profile_mut(&name);
            if let Some(default_host) = default_host {
                profile.default_host = Some(default_host);
            }
            if let Some(default_repo) = default_repo {
                profile.default_repo = Some(default_repo);
            }

            config.save()?;
            println!("✓ Updated profile {}", name);
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
        ConfigUnsetKey::DefaultProfile => {
            if config.default_profile.is_none() {
                println!("No default profile configured.");
                return Ok(());
            }
            config.default_profile = None;
            config.save()?;
            println!("✓ Cleared default profile");
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

fn summarize_hosts(hosts: &HashMap<String, HostConfig>) -> BTreeMap<String, HostSummary> {
    hosts
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
        .collect()
}

fn summarize_profiles(
    profiles: &HashMap<String, ProfileConfig>,
) -> BTreeMap<String, ProfileOutput> {
    profiles
        .iter()
        .map(|(name, profile)| (name.clone(), profile_output(name, profile)))
        .collect()
}

fn profile_output(name: &str, profile: &ProfileConfig) -> ProfileOutput {
    ProfileOutput {
        name: name.to_string(),
        default_host: profile.default_host.clone(),
        default_repo: profile.default_repo.clone(),
        hosts: summarize_hosts(&profile.hosts),
    }
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
