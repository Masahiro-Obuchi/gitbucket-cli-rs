//! Finite, configurable deadlines shared by REST, Web and Git.
use std::time::Duration;

use crate::error::{GbError, Result};

pub fn duration(name: &str, default_seconds: u64) -> Result<Duration> {
    let Some(value) = std::env::var_os(name) else {
        return Ok(Duration::from_secs(default_seconds));
    };
    let invalid = || {
        GbError::Config(format!(
            "{name} must be a positive number of seconds (at most 86400)."
        ))
    };
    let seconds: f64 = value
        .to_str()
        .ok_or_else(invalid)?
        .parse()
        .map_err(|_| invalid())?;
    if !seconds.is_finite() || !(0.001..=86400.0).contains(&seconds) {
        return Err(invalid());
    }
    Duration::try_from_secs_f64(seconds).map_err(|_| invalid())
}

pub fn http_builder() -> Result<reqwest::ClientBuilder> {
    Ok(reqwest::Client::builder()
        .connect_timeout(duration("GB_HTTP_CONNECT_TIMEOUT", 10)?)
        .read_timeout(duration("GB_HTTP_READ_TIMEOUT", 30)?)
        .timeout(duration("GB_HTTP_TIMEOUT", 120)?)
        // Even protocol-level automatic retries are disabled: some operations
        // are writes, including GitBucket's milestone state-changing GET.
        .retry(reqwest::retry::never()))
}
