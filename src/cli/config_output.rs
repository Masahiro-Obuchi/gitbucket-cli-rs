use std::collections::{BTreeMap, HashMap};
use std::path::Path;

use serde::Serialize;

use crate::cli::config::HostField;
use crate::config::auth::{AuthConfig, HostConfig, ProfileConfig};
use crate::error::Result;
use crate::output;

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

pub(crate) fn print_list(config: &AuthConfig, path: &Path, json: bool) -> Result<()> {
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
        return output::print_json(&payload);
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
            print_host_summary(&host, "  ");
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
                    print_host_summary(&host, "    ");
                }
            }
        }
    }

    Ok(())
}

pub(crate) fn print_host(
    hostname: &str,
    host: &HostConfig,
    field: Option<&HostField>,
    json: bool,
) -> Result<()> {
    let output = HostOutput {
        hostname: hostname.to_string(),
        user: host.user.clone(),
        protocol: host.protocol.clone(),
        has_token: !host.token.is_empty(),
    };

    if json {
        return output::print_json(&output);
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

    Ok(())
}

pub(crate) fn print_profile(name: &str, profile: &ProfileConfig, json: bool) -> Result<()> {
    let output = profile_output(name, profile);

    if json {
        return output::print_json(&output);
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
            print_host_summary(&host, "  ");
        }
    }

    Ok(())
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

fn print_host_summary(host: &HostSummary, indent: &str) {
    println!("{}User: {}", indent, display_user(&host.user));
    println!("{}Protocol: {}", indent, host.protocol);
    println!(
        "{}Token: {}",
        indent,
        if host.has_token {
            "configured"
        } else {
            "missing"
        }
    );
}

fn display_user(user: &str) -> &str {
    if user.is_empty() {
        "<unset>"
    } else {
        user
    }
}
