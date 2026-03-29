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
        let url = format!("{}{}", self.base_url, path);
        let resp = self.client.get(&url).send().await?;
        self.handle_response(resp).await
    }

    /// Make a POST request with a JSON body
    pub async fn post<T: DeserializeOwned, B: serde::Serialize>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<T> {
        let url = format!("{}{}", self.base_url, path);
        let resp = self.client.post(&url).json(body).send().await?;
        self.handle_response(resp).await
    }

    /// Make a PATCH request with a JSON body
    pub async fn patch<T: DeserializeOwned, B: serde::Serialize>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<T> {
        let url = format!("{}{}", self.base_url, path);
        let resp = self.client.patch(&url).json(body).send().await?;
        self.handle_response(resp).await
    }

    /// Make a DELETE request
    pub async fn delete(&self, path: &str) -> Result<()> {
        let url = format!("{}{}", self.base_url, path);
        let resp = self.client.delete(&url).send().await?;
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

    /// Make a PUT request with a JSON body
    pub async fn put<T: DeserializeOwned, B: serde::Serialize>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<T> {
        let url = format!("{}{}", self.base_url, path);
        let resp = self.client.put(&url).json(body).send().await?;
        self.handle_response(resp).await
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

    use super::{normalize_base_url, normalize_web_base_url, parse_success_body, ApiClient};

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
