use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, AUTHORIZATION};
use reqwest::{Client, Method, Response};
use serde::de::DeserializeOwned;
use serde_json::Value;

pub(crate) use crate::api::url::{normalize_base_url, normalize_web_base_url};
use crate::api::url::{normalize_origin_path_or_url, normalize_path_or_url};
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

    /// Make a GET request for text from the configured GitBucket origin.
    pub async fn get_text_from_origin(&self, path_or_url: &str) -> Result<String> {
        let url = normalize_origin_path_or_url(path_or_url, &self.base_url)?;
        let resp = self.client.get(&url).send().await?;
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        if status.is_success() {
            Ok(body)
        } else {
            Err(GbError::Api {
                status: status.as_u16(),
                message: body,
            })
        }
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

#[cfg(test)]
mod tests {
    use serde::Deserialize;

    use super::parse_success_body;

    #[derive(Debug, Deserialize, PartialEq)]
    struct WrappedValue {
        name: String,
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
