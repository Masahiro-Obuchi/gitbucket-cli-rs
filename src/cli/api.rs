use std::fs;
use std::io::Read;

use clap::Args;
use reqwest::Method;
use serde_json::Value;

use crate::cli::common::{create_client, resolve_hostname};
use crate::error::{GbError, Result};

#[derive(Args)]
pub struct ApiArgs {
    /// API endpoint path relative to /api/v3, or a full URL
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
    let client = create_client(&hostname)?;
    let method = resolve_method(args.method.as_deref(), args.input.is_some())?;
    let endpoint = normalize_endpoint(&args.endpoint);
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

fn normalize_endpoint(endpoint: &str) -> String {
    let trimmed = endpoint.trim();
    if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        return trimmed.to_string();
    }

    let without_api_prefix = trimmed
        .strip_prefix("/api/v3")
        .or_else(|| trimmed.strip_prefix("api/v3"))
        .unwrap_or(trimmed);

    if without_api_prefix.is_empty() || without_api_prefix == "/" {
        "/".into()
    } else if without_api_prefix.starts_with('/') {
        without_api_prefix.to_string()
    } else {
        format!("/{}", without_api_prefix)
    }
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

    use super::{normalize_endpoint, resolve_method};

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
        assert_eq!(normalize_endpoint("user"), "/user");
    }

    #[test]
    fn strips_api_prefix_from_endpoint() {
        assert_eq!(normalize_endpoint("/api/v3/user"), "/user");
        assert_eq!(normalize_endpoint("api/v3/user"), "/user");
    }

    #[test]
    fn preserves_absolute_url_endpoint() {
        assert_eq!(
            normalize_endpoint("https://gitbucket.example.com/gitbucket/api/v3/user"),
            "https://gitbucket.example.com/gitbucket/api/v3/user"
        );
    }
}
