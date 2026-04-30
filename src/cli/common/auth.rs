use crate::api::client::ApiClient;
use crate::config::auth::{AuthConfig, HostConfig};
use crate::error::{GbError, Result};

/// Resolve GitBucket host or URL from CLI arg, env var, or config.
pub fn resolve_hostname(
    cli_hostname: &Option<String>,
    cli_profile: &Option<String>,
) -> Result<String> {
    let config = AuthConfig::load()?;
    let hostname = config.resolve_hostname(cli_hostname.as_deref(), cli_profile.as_deref())?;
    let has_active_profile = config
        .active_profile_name(cli_profile.as_deref())?
        .is_some();
    hostname.ok_or_else(|| {
        if has_active_profile {
            GbError::Auth(
                "No GitBucket host or URL configured for the selected profile. Pass --hostname or set the profile default host.".into(),
            )
        } else {
            GbError::Auth("No GitBucket host or URL configured. Run `gb auth login` first.".into())
        }
    })
}

/// Validate a selected profile even for commands that do not otherwise need config.
pub fn validate_selected_profile(cli_profile: &Option<String>) -> Result<()> {
    let config = AuthConfig::load()?;
    config.active_profile_name(cli_profile.as_deref())?;
    Ok(())
}

pub fn resolve_host_config(hostname: &str, cli_profile: &Option<String>) -> Result<HostConfig> {
    let config = AuthConfig::load()?;
    config.get_host_for_profile(hostname, cli_profile.as_deref())
}

/// Create an API client from config.
pub fn create_client(hostname: &str, cli_profile: &Option<String>) -> Result<ApiClient> {
    let host = resolve_host_config(hostname, cli_profile)?;
    ApiClient::new(hostname, &host.token, &host.protocol)
}
