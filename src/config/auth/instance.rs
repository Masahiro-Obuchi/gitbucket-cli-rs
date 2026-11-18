//! Normalize targets at input and look up credentials by canonical base URL.
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
    // Removing the API suffix can expose trailing slashes. Store a URL
    // that normalizes to itself when loaded on the next invocation.
    let base = base.trim_end_matches('/');
    if base.ends_with("/api/v3") {
        return Err(invalid());
    }
    crate::api::url::normalize_origin_path_or_url(base, base).map_err(|_| invalid())?;
    Ok(base.to_owned())
}

pub(crate) fn instance_protocol(instance: &str) -> &str {
    instance
        .split_once("://")
        .expect("canonical instance URL")
        .0
}

pub(super) fn validate_saved_instance(value: &str) -> Result<()> {
    if !value.contains("://") || instance_key(value, "https")? != value {
        return Err(GbError::Config(
            "Saved GitBucket instances must use normalized HTTP(S) base URLs. Use a fresh GB_CONFIG_DIR and run `gb auth login` to set up credentials again.".into(),
        ));
    }
    Ok(())
}

pub(super) fn instance_candidates(
    target: &str,
    protocol: Option<&str>,
) -> Result<(String, Option<String>)> {
    if target.contains("://") {
        return Ok((instance_key(target, "https")?, None));
    }
    let preferred = instance_key(target, protocol.unwrap_or("https"))?;
    let alternate = if protocol.is_none() {
        Some(instance_key(target, "http")?)
    } else {
        None
    };
    Ok((preferred, alternate))
}

pub(super) fn matching_instance<'a>(
    hosts: &HashMap<String, HostConfig>,
    candidates: &'a (String, Option<String>),
) -> Result<Option<&'a str>> {
    let (preferred, alternate) = candidates;
    let preferred = hosts.contains_key(preferred).then_some(preferred.as_str());
    let alternate = alternate.as_deref().filter(|url| hosts.contains_key(*url));
    if preferred.is_some() && alternate.is_some() {
        return Err(GbError::Config(
            "The saved instance scheme is ambiguous. Specify an explicit HTTP(S) URL.".into(),
        ));
    }
    Ok(preferred.or(alternate))
}
