use std::fs;
use std::io::Read;

use clap::Args;
use reqwest::Method;
use serde_json::Value;
use url::Url;

use crate::api::client::{normalize_base_url, ApiClient};
use crate::cli::common::{resolve_host_config, resolve_hostname};
use crate::error::{GbError, Result};

#[derive(Args)]
pub struct ApiArgs {
    /// API endpoint path relative to /api/v3, or a full URL under the configured GitBucket API base
    pub endpoint: String,

    /// HTTP method to use
    #[arg(short = 'X', long)]
    pub method: Option<String>,

    /// JSON request body file path, or `-` to read from stdin
    #[arg(short = 'i', long)]
    pub input: Option<String>,
}

pub async fn run(args: ApiArgs, cli_hostname: &Option<String>) -> Result<()> {
    let hostname = resolve_hostname(cli_hostname)?;
    let host = resolve_host_config(&hostname)?;
    let client = ApiClient::new(&hostname, &host.token, &host.protocol)?;
    let method = resolve_method(args.method.as_deref(), args.input.is_some())?;
    let allowed_base_url = normalize_base_url(&hostname, &host.protocol)?;
    let endpoint = normalize_endpoint(&args.endpoint, &allowed_base_url)?;
    let body = match args.input {
        Some(input) => Some(read_json_input(&input)?),
        None => None,
    };

    let response = client.raw_request(method, &endpoint, body.as_ref()).await?;
    print_response(&response)?;
    Ok(())
}

fn resolve_method(method: Option<&str>, has_input: bool) -> Result<Method> {
    match method {
        Some(value) => value.parse::<Method>().map_err(|_| {
            GbError::Other(format!(
                "Invalid HTTP method '{}'. Expected a valid method such as GET, POST, PATCH, PUT, or DELETE",
                value
            ))
        }),
        None if has_input => Ok(Method::POST),
        None => Ok(Method::GET),
    }
}

fn normalize_endpoint(endpoint: &str, allowed_base_url: &str) -> Result<String> {
    let trimmed = endpoint.trim();
    if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        validate_absolute_endpoint(trimmed, allowed_base_url)?;
        return Ok(trimmed.to_string());
    }

    let without_api_prefix = trimmed
        .strip_prefix("/api/v3")
        .or_else(|| trimmed.strip_prefix("api/v3"))
        .unwrap_or(trimmed);

    if without_api_prefix.is_empty() || without_api_prefix == "/" {
        Ok("/".into())
    } else if without_api_prefix.starts_with('/') {
        Ok(without_api_prefix.to_string())
    } else {
        Ok(format!("/{}", without_api_prefix))
    }
}

fn validate_absolute_endpoint(endpoint: &str, allowed_base_url: &str) -> Result<()> {
    let endpoint_url = Url::parse(endpoint)?;
    let allowed_url = Url::parse(allowed_base_url)?;

    let same_origin = endpoint_url.scheme() == allowed_url.scheme()
        && endpoint_url.host_str() == allowed_url.host_str()
        && endpoint_url.port_or_known_default() == allowed_url.port_or_known_default();
    let same_api_base = path_has_base_prefix(endpoint_url.path(), allowed_url.path());

    if same_origin && same_api_base {
        Ok(())
    } else {
        Err(GbError::Other(format!(
            "Absolute URLs must stay within the configured GitBucket API base {}. Use a relative path like `user` or change --hostname if you intend a different instance.",
            allowed_base_url
        )))
    }
}

fn path_has_base_prefix(path: &str, base: &str) -> bool {
    let normalized_base = base.trim_end_matches('/');
    if normalized_base.is_empty() {
        return true;
    }

    path == normalized_base
        || path
            .strip_prefix(normalized_base)
            .is_some_and(|rest| rest.is_empty() || rest.starts_with('/'))
}

fn read_json_input(input: &str) -> Result<Value> {
    let raw = if input == "-" {
        let mut raw = String::new();
        std::io::stdin().read_to_string(&mut raw)?;
        raw
    } else {
        fs::read_to_string(input)?
    };

    Ok(serde_json::from_str(&raw)?)
}

fn print_response(value: &Value) -> Result<()> {
    match value {
        Value::String(text) => println!("{}", text),
        other => println!("{}", serde_json::to_string_pretty(other)?),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use reqwest::Method;

    use super::{
        normalize_endpoint, path_has_base_prefix, resolve_method, validate_absolute_endpoint,
    };

    #[test]
    fn resolves_default_get_without_input() {
        assert_eq!(resolve_method(None, false).unwrap(), Method::GET);
    }

    #[test]
    fn resolves_default_post_when_input_is_present() {
        assert_eq!(resolve_method(None, true).unwrap(), Method::POST);
    }

    #[test]
    fn normalizes_relative_endpoint() {
        assert_eq!(
            normalize_endpoint("user", "https://gitbucket.example.com/api/v3").unwrap(),
            "/user"
        );
    }

    #[test]
    fn strips_api_prefix_from_endpoint() {
        assert_eq!(
            normalize_endpoint("/api/v3/user", "https://gitbucket.example.com/api/v3").unwrap(),
            "/user"
        );
        assert_eq!(
            normalize_endpoint("api/v3/user", "https://gitbucket.example.com/api/v3").unwrap(),
            "/user"
        );
    }

    #[test]
    fn preserves_absolute_url_within_same_api_base() {
        assert_eq!(
            normalize_endpoint(
                "https://gitbucket.example.com/gitbucket/api/v3/user",
                "https://gitbucket.example.com/gitbucket/api/v3"
            )
            .unwrap(),
            "https://gitbucket.example.com/gitbucket/api/v3/user"
        );
    }

    #[test]
    fn rejects_absolute_url_on_another_host() {
        let error = validate_absolute_endpoint(
            "https://evil.example.com/api/v3/user",
            "https://gitbucket.example.com/api/v3",
        )
        .unwrap_err();
        assert!(error.to_string().contains("configured GitBucket API base"));
    }

    #[test]
    fn rejects_absolute_url_outside_the_configured_subpath() {
        let error = validate_absolute_endpoint(
            "https://gitbucket.example.com/other/api/v3/user",
            "https://gitbucket.example.com/gitbucket/api/v3",
        )
        .unwrap_err();
        assert!(error.to_string().contains("configured GitBucket API base"));
    }

    #[test]
    fn path_prefix_check_requires_path_boundary() {
        assert!(path_has_base_prefix(
            "/gitbucket/api/v3/user",
            "/gitbucket/api/v3"
        ));
        assert!(!path_has_base_prefix(
            "/gitbucket/api/v3evil/user",
            "/gitbucket/api/v3"
        ));
    }
}
