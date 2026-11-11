//! Credential keys retain scheme, port and deployment; aliases never widen scope.
use std::collections::HashMap;

use super::HostConfig;
use crate::error::{GbError, Result};

pub(crate) fn instance_key(host: &str, protocol: &str) -> Result<String> {
    let invalid = || {
        GbError::Config(
        "Invalid GitBucket instance. Use an HTTP(S) host/base URL without credentials, query, fragment or ambiguous path.".into(),
    )
    };
    if !matches!(protocol, "http" | "https") || host.chars().any(char::is_control) {
        return Err(invalid());
    }
    let base = crate::api::url::normalize_web_base_url(host, protocol).map_err(|_| invalid())?;
    crate::api::url::normalize_origin_path_or_url(&base, &base).map_err(|_| invalid())?;
    Ok(base)
}

pub(super) fn stored_instance(key: &str, host: &HostConfig) -> Result<String> {
    instance_key(key, &host.protocol)
}

pub(super) fn matching_keys(
    hosts: &HashMap<String, HostConfig>,
    target: &str,
    protocol: Option<&str>,
) -> Result<Vec<String>> {
    // Validate even an empty table, before any credential is selected.
    instance_key(target, protocol.unwrap_or("https"))?;
    let mut matches = Vec::new();
    for (key, host) in hosts {
        let stored = stored_instance(key, host)?;
        let scheme = protocol.unwrap_or_else(|| {
            if stored.starts_with("http://") {
                "http"
            } else {
                "https"
            }
        });
        if instance_key(target, scheme)? == stored {
            matches.push(key.clone());
        }
    }
    matches.sort();
    Ok(matches)
}

pub(super) fn unique_key(
    hosts: &HashMap<String, HostConfig>,
    target: &str,
    protocol: Option<&str>,
) -> Result<Option<String>> {
    let matches = matching_keys(hosts, target, protocol)?;
    if matches.len() > 1 {
        return Err(GbError::Config(
            "Ambiguous saved credentials for this instance. Specify an explicit scheme or inspect `gb config migrate`; conflicting aliases must be resolved before use.".into(),
        ));
    }
    Ok(matches.into_iter().next())
}
