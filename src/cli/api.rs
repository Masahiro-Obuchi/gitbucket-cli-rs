use std::fs;
use std::io::Read;

use clap::Args;
use reqwest::Method;
use serde_json::Value;

use crate::api::url::normalize_path_or_url;
use crate::cli::common::HostContext;
use crate::error::{GbError, Result};
use crate::output;

#[derive(Args)]
#[command(
    after_help = "Endpoints must remain within the selected API base after URL normalization. Credentials, fragments, and paths escaping that base are rejected. Authenticated API requests do not follow redirects; use the direct GitBucket base URL."
)]
pub struct ApiArgs {
    /// API endpoint path relative to /api/v3, or a full URL under the configured GitBucket API base
    #[arg(value_name = "ENDPOINT")]
    pub endpoint: String,

    /// HTTP method to use (defaults to GET, or POST when --input is used)
    #[arg(short = 'X', long, value_name = "METHOD")]
    pub method: Option<String>,

    /// JSON request body file path, or `-` to read from stdin
    #[arg(short = 'i', long, value_name = "FILE|-")]
    pub input: Option<String>,
}

pub async fn run(
    args: ApiArgs,
    cli_hostname: &Option<String>,
    cli_profile: &Option<String>,
) -> Result<()> {
    let ctx = HostContext::resolve(cli_hostname, cli_profile)?;
    let method = resolve_method(args.method.as_deref(), args.input.is_some())?;
    let endpoint = normalize_endpoint(&args.endpoint, ctx.client.base_url())?;
    let body = match args.input {
        Some(input) => Some(read_json_input(&input)?),
        None => None,
    };

    let response = ctx
        .client
        .raw_request(method, &endpoint, body.as_ref())
        .await?;
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
    if url::Url::parse(trimmed).is_ok() {
        validate_absolute_endpoint(trimmed, allowed_base_url)?;
        return Ok(trimmed.to_string());
    }

    let without_api_prefix = strip_api_prefix(trimmed).unwrap_or(trimmed);

    let normalized = if without_api_prefix.is_empty() || without_api_prefix == "/" {
        "/".to_string()
    } else if without_api_prefix.starts_with('/') || without_api_prefix.starts_with('?') {
        without_api_prefix.to_string()
    } else {
        format!("/{}", without_api_prefix)
    };
    normalize_path_or_url(&normalized, allowed_base_url)?;
    Ok(normalized)
}

fn strip_api_prefix(endpoint: &str) -> Option<&str> {
    strip_prefix_with_boundary(endpoint, "/api/v3")
        .or_else(|| strip_prefix_with_boundary(endpoint, "api/v3"))
}

fn strip_prefix_with_boundary<'a>(value: &'a str, prefix: &str) -> Option<&'a str> {
    value
        .strip_prefix(prefix)
        .filter(|rest| rest.is_empty() || rest.starts_with('/') || rest.starts_with('?'))
}

fn validate_absolute_endpoint(endpoint: &str, allowed_base_url: &str) -> Result<()> {
    normalize_path_or_url(endpoint, allowed_base_url)?;
    Ok(())
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
    output::print_json(value)
}

#[cfg(test)]
mod tests {
    use reqwest::Method;

    use super::{normalize_endpoint, resolve_method, strip_api_prefix, validate_absolute_endpoint};

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
    fn does_not_strip_similar_api_prefixes_without_boundary() {
        assert_eq!(strip_api_prefix("/api/v30/user"), None);
        assert_eq!(strip_api_prefix("api/v30/user"), None);
        assert_eq!(
            normalize_endpoint("/api/v30/user", "https://gitbucket.example.com/api/v3").unwrap(),
            "/api/v30/user"
        );
        assert_eq!(
            normalize_endpoint("api/v30/user", "https://gitbucket.example.com/api/v3").unwrap(),
            "/api/v30/user"
        );
    }

    #[test]
    fn strips_api_prefix_before_root_query() {
        assert_eq!(
            strip_api_prefix("/api/v3?per_page=50"),
            Some("?per_page=50")
        );
        assert_eq!(strip_api_prefix("api/v3?per_page=50"), Some("?per_page=50"));
        assert_eq!(
            normalize_endpoint(
                "/api/v3?per_page=50",
                "https://gitbucket.example.com/api/v3"
            )
            .unwrap(),
            "?per_page=50"
        );
        assert_eq!(
            normalize_endpoint("api/v3?per_page=50", "https://gitbucket.example.com/api/v3")
                .unwrap(),
            "?per_page=50"
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
}
