use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, AUTHORIZATION};
use reqwest::{Client, Method, Response};
use serde::de::DeserializeOwned;
use serde_json::Value;
use url::Url;

use crate::error::{GbError, Result};

/// GitBucket API client
#[derive(Debug, Clone)]
pub struct ApiClient {
    client: Client,
    base_url: String,
}

impl ApiClient {
    /// Create a new API client
    pub fn new(hostname: &str, token: &str, protocol: &str) -> Result<Self> {
        let mut headers = HeaderMap::new();
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&format!("token {}", token))
                .map_err(|e| GbError::Other(format!("Invalid token: {}", e)))?,
        );
        headers.insert(ACCEPT, HeaderValue::from_static("application/json"));

        let client = Client::builder().default_headers(headers).build()?;
        let base_url = normalize_base_url(hostname, protocol)?;

        Ok(Self { client, base_url })
    }

    /// Make a GET request and deserialize the response
    pub async fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T> {
        self.request_json(Method::GET, path, None::<&()>).await
    }

    /// Make a GET request and return response headers with the deserialized body.
    pub(crate) async fn get_with_headers<T: DeserializeOwned>(
        &self,
        path_or_url: &str,
    ) -> Result<(T, HeaderMap)> {
        let url = normalize_path_or_url(path_or_url, &self.base_url)?;
        let resp = self.client.get(&url).send().await?;
        let status = resp.status();
        let headers = resp.headers().clone();
        let body = resp.text().await.unwrap_or_default();
        if status.is_success() {
            Ok((parse_success_body(&body)?, headers))
        } else {
            Err(GbError::Api {
                status: status.as_u16(),
                message: body,
            })
        }
    }

    /// Make a POST request with a JSON body
    pub async fn post<T: DeserializeOwned, B: serde::Serialize>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<T> {
        self.request_json(Method::POST, path, Some(body)).await
    }

    /// Make a PATCH request with a JSON body
    pub async fn patch<T: DeserializeOwned, B: serde::Serialize>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<T> {
        self.request_json(Method::PATCH, path, Some(body)).await
    }

    /// Make a DELETE request
    pub async fn delete(&self, path: &str) -> Result<()> {
        let url = self.api_url(path);
        let resp = self.client.delete(&url).send().await?;
        self.handle_empty_response(resp).await
    }

    /// Make a PUT request with a JSON body
    pub async fn put<T: DeserializeOwned, B: serde::Serialize>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<T> {
        self.request_json(Method::PUT, path, Some(body)).await
    }

    /// Make a raw request for the `gb api` command
    pub async fn raw_request(
        &self,
        method: Method,
        path: &str,
        body: Option<&serde_json::Value>,
    ) -> Result<serde_json::Value> {
        let url = if path.starts_with("http://") || path.starts_with("https://") {
            path.to_string()
        } else {
            format!("{}{}", self.base_url, path)
        };

        let mut req = self.client.request(method, &url);
        if let Some(body) = body {
            req = req.json(body);
        }
        let resp = req.send().await?;
        let status = resp.status();
        let body = resp.text().await?;
        if status.is_success() {
            if body.trim().is_empty() {
                Ok(Value::Null)
            } else {
                Ok(serde_json::from_str(&body)?)
            }
        } else {
            Err(GbError::Api {
                status: status.as_u16(),
                message: body,
            })
        }
    }

    /// Get the base URL for constructing web URLs
    pub fn web_url(&self, path: &str) -> String {
        let base = self.base_url.trim_end_matches("/api/v3");
        format!("{}{}", base, path)
    }

    async fn handle_response<T: DeserializeOwned>(&self, resp: Response) -> Result<T> {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        if status.is_success() {
            parse_success_body(&body)
        } else {
            Err(GbError::Api {
                status: status.as_u16(),
                message: body,
            })
        }
    }

    async fn request_json<T: DeserializeOwned, B: serde::Serialize>(
        &self,
        method: Method,
        path: &str,
        body: Option<&B>,
    ) -> Result<T> {
        let url = self.api_url(path);
        let mut request = self.client.request(method, &url);
        if let Some(body) = body {
            request = request.json(body);
        }
        let resp = request.send().await?;
        self.handle_response(resp).await
    }

    async fn handle_empty_response(&self, resp: Response) -> Result<()> {
        let status = resp.status();
        if status.is_success() {
            Ok(())
        } else {
            let message = resp.text().await.unwrap_or_default();
            Err(GbError::Api {
                status: status.as_u16(),
                message,
            })
        }
    }

    fn api_url(&self, path: &str) -> String {
        format!("{}{}", self.base_url, path)
    }
}

fn normalize_path_or_url(path_or_url: &str, base_url: &str) -> Result<String> {
    if path_or_url.starts_with("http://") || path_or_url.starts_with("https://") {
        validate_absolute_api_url(path_or_url, base_url)?;
        Ok(path_or_url.to_string())
    } else {
        Ok(format!(
            "{}{}",
            base_url,
            normalize_relative_api_path(path_or_url, base_url)
        ))
    }
}

fn validate_absolute_api_url(url: &str, base_url: &str) -> Result<()> {
    let target = Url::parse(url)?;
    let base = Url::parse(base_url)?;

    let same_origin = target.scheme() == base.scheme()
        && target.host_str() == base.host_str()
        && target.port_or_known_default() == base.port_or_known_default();
    let same_api_base = path_has_base_prefix(target.path(), base.path());

    if same_origin && same_api_base {
        Ok(())
    } else {
        Err(GbError::Other(format!(
            "Refusing to follow pagination URL outside configured GitBucket API base {}",
            base_url
        )))
    }
}

fn normalize_relative_api_path<'a>(path: &'a str, base_url: &str) -> &'a str {
    if !path.starts_with('/') {
        return path;
    }

    let Ok(base) = Url::parse(base_url) else {
        return path;
    };
    let api_path = base.path().trim_end_matches('/');
    if let Some(stripped) = strip_prefix_with_boundary(path, api_path) {
        return ensure_request_path(stripped);
    }
    if let Some(stripped) = strip_prefix_with_boundary(path, "/api/v3") {
        return ensure_request_path(stripped);
    }

    path
}

fn strip_prefix_with_boundary<'a>(path: &'a str, prefix: &str) -> Option<&'a str> {
    path.strip_prefix(prefix).filter(|rest| {
        rest.is_empty() || rest.starts_with('/') || rest.starts_with('?') || rest.starts_with('#')
    })
}

fn ensure_request_path(path: &str) -> &str {
    if path.is_empty() {
        ""
    } else if path.starts_with('/') || path.starts_with('?') || path.starts_with('#') {
        path
    } else {
        ""
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

fn parse_success_body<T: DeserializeOwned>(body: &str) -> Result<T> {
    let value: Value = serde_json::from_str(body)?;

    if let Some(inner) = value.as_str() {
        return Ok(serde_json::from_str(inner)?);
    }

    if value.get("status").is_some() {
        if let Some(inner) = value.get("body") {
            return match inner {
                Value::String(text) => Ok(serde_json::from_str(text)?),
                other => Ok(serde_json::from_value(other.clone())?),
            };
        }
    }

    Ok(serde_json::from_value(value)?)
}

pub(crate) fn normalize_base_url(hostname: &str, protocol: &str) -> Result<String> {
    let input = hostname.trim().trim_end_matches('/');
    let candidate = if input.starts_with("http://") || input.starts_with("https://") {
        input.to_string()
    } else {
        format!("{}://{}", protocol, input)
    };

    let parsed = Url::parse(&candidate)?;
    let host = parsed
        .host_str()
        .ok_or_else(|| GbError::Config(format!("Invalid GitBucket host or URL: {}", hostname)))?;

    let mut base = format!("{}://{}", parsed.scheme(), host);
    if let Some(port) = parsed.port() {
        base.push_str(&format!(":{}", port));
    }

    let path = parsed.path().trim_end_matches('/');
    if !path.is_empty() && path != "/" {
        base.push_str(path);
    }

    if base.ends_with("/api/v3") {
        Ok(base)
    } else {
        Ok(format!("{}/api/v3", base))
    }
}

pub(crate) fn normalize_web_base_url(hostname: &str, protocol: &str) -> Result<String> {
    Ok(normalize_base_url(hostname, protocol)?
        .trim_end_matches("/api/v3")
        .to_string())
}

#[cfg(test)]
mod tests {
    use serde::Deserialize;

    use super::{
        normalize_base_url, normalize_path_or_url, normalize_relative_api_path,
        normalize_web_base_url, parse_success_body, ApiClient,
    };

    #[derive(Debug, Deserialize, PartialEq)]
    struct WrappedValue {
        name: String,
    }

    #[test]
    fn normalizes_plain_hostname() {
        let base = normalize_base_url("gitbucket.example.com", "https").unwrap();
        assert_eq!(base, "https://gitbucket.example.com/api/v3");
    }

    #[test]
    fn normalizes_hostname_with_path() {
        let base = normalize_base_url("gitbucket.example.com/gitbucket", "https").unwrap();
        assert_eq!(base, "https://gitbucket.example.com/gitbucket/api/v3");
    }

    #[test]
    fn normalizes_full_base_url() {
        let base = normalize_base_url("https://gitbucket.example.com/gitbucket", "http").unwrap();
        assert_eq!(base, "https://gitbucket.example.com/gitbucket/api/v3");
    }

    #[test]
    fn keeps_existing_api_base_url() {
        let base =
            normalize_base_url("https://gitbucket.example.com/gitbucket/api/v3", "https").unwrap();
        assert_eq!(base, "https://gitbucket.example.com/gitbucket/api/v3");
    }

    #[test]
    fn builds_web_url_from_subpath_base() {
        let client = ApiClient::new(
            "https://gitbucket.example.com/gitbucket",
            "dummy-token",
            "https",
        )
        .unwrap();
        assert_eq!(
            client.web_url("/alice/my-repo"),
            "https://gitbucket.example.com/gitbucket/alice/my-repo"
        );
    }

    #[test]
    fn preserves_port_and_trailing_slash_in_base_url() {
        let base =
            normalize_base_url("https://gitbucket.example.com:8443/gitbucket/", "http").unwrap();
        assert_eq!(base, "https://gitbucket.example.com:8443/gitbucket/api/v3");
    }

    #[test]
    fn normalizes_web_base_url() {
        let base = normalize_web_base_url("gitbucket.example.com/gitbucket", "https").unwrap();
        assert_eq!(base, "https://gitbucket.example.com/gitbucket");
    }

    #[test]
    fn normalizes_api_root_relative_pagination_path() {
        assert_eq!(
            normalize_relative_api_path(
                "/api/v3/repos/alice/project/issues/7/comments?page=2",
                "https://gitbucket.example.com/api/v3"
            ),
            "/repos/alice/project/issues/7/comments?page=2"
        );
    }

    #[test]
    fn normalizes_subpath_api_root_relative_pagination_path() {
        assert_eq!(
            normalize_relative_api_path(
                "/gitbucket/api/v3/repos/alice/project/issues/7/comments?page=2",
                "https://gitbucket.example.com/gitbucket/api/v3"
            ),
            "/repos/alice/project/issues/7/comments?page=2"
        );
    }

    #[test]
    fn leaves_plain_root_relative_paths_unchanged() {
        assert_eq!(
            normalize_relative_api_path(
                "/repos/alice/project/issues/7/comments?page=2",
                "https://gitbucket.example.com/api/v3"
            ),
            "/repos/alice/project/issues/7/comments?page=2"
        );
    }

    #[test]
    fn accepts_absolute_pagination_url_inside_api_base() {
        let normalized = normalize_path_or_url(
            "https://gitbucket.example.com/gitbucket/api/v3/repos/alice/project/issues/7/comments?page=2",
            "https://gitbucket.example.com/gitbucket/api/v3",
        )
        .unwrap();

        assert_eq!(
            normalized,
            "https://gitbucket.example.com/gitbucket/api/v3/repos/alice/project/issues/7/comments?page=2"
        );
    }

    #[test]
    fn rejects_absolute_pagination_url_on_different_host() {
        let err = normalize_path_or_url(
            "https://attacker.example/repos/alice/project/issues/7/comments?page=2",
            "https://gitbucket.example.com/api/v3",
        )
        .unwrap_err()
        .to_string();

        assert!(err.contains("outside configured GitBucket API base"));
    }

    #[test]
    fn rejects_absolute_pagination_url_outside_subpath_api_base() {
        let err = normalize_path_or_url(
            "https://gitbucket.example.com/api/v3/repos/alice/project/issues/7/comments?page=2",
            "https://gitbucket.example.com/gitbucket/api/v3",
        )
        .unwrap_err()
        .to_string();

        assert!(err.contains("outside configured GitBucket API base"));
    }

    #[test]
    fn rejects_absolute_pagination_url_with_api_prefix_boundary_mismatch() {
        let err = normalize_path_or_url(
            "https://gitbucket.example.com/api/v30/repos/alice/project/issues/7/comments?page=2",
            "https://gitbucket.example.com/api/v3",
        )
        .unwrap_err()
        .to_string();

        assert!(err.contains("outside configured GitBucket API base"));
    }

    #[test]
    fn parses_string_wrapped_json_response() {
        let parsed: WrappedValue = parse_success_body(r#""{\"name\":\"wrapped\"}""#).unwrap();
        assert_eq!(
            parsed,
            WrappedValue {
                name: "wrapped".into()
            }
        );
    }

    #[test]
    fn parses_enveloped_body_json_response() {
        let parsed: WrappedValue =
            parse_success_body(r#"{"status":200,"body":"{\"name\":\"wrapped\"}","headers":{}}"#)
                .unwrap();
        assert_eq!(
            parsed,
            WrappedValue {
                name: "wrapped".into()
            }
        );
    }
}
