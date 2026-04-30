use std::collections::BTreeMap;

use serde::Serialize;

use crate::config::auth::{AuthConfig, HostConfig, ProfileConfig};
use crate::error::{GbError, Result};
use crate::output;

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

pub(crate) fn print_status(
    config: &AuthConfig,
    cli_hostname: &Option<String>,
    active_profile: Option<&str>,
    cli_profile: &Option<String>,
    json: bool,
) -> Result<()> {
    let profile_filter = selected_profile_filter(active_profile, cli_profile);
    let output = status_output(config, cli_hostname, active_profile, profile_filter)?;

    if json {
        return output::print_json(&output);
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
    let effective_actor = effective_actor(config, cli_hostname, active_profile)?;

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
    profile: &ProfileConfig,
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
