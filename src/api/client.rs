use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, AUTHORIZATION};
use reqwest::{Client, Method};
use serde::de::DeserializeOwned;
use serde_json::Value;

use super::transport::{self, Secrets};
pub(crate) use crate::api::url::{normalize_base_url, normalize_web_base_url};
use crate::api::url::{normalize_origin_path_or_url, normalize_path_or_url};
use crate::error::{GbError, Result};

#[derive(Clone, Copy)]
pub(crate) enum JsonContract {
    Standard,
    PullRequestCreate,
    PullRequestMerge,
}

#[derive(Debug, Clone)]
pub struct ApiClient {
    client: Client,
    base_url: String,
    secrets: Secrets,
}

impl ApiClient {
    pub fn new(hostname: &str, token: &str, protocol: &str) -> Result<Self> {
        let mut headers = HeaderMap::new();
        let mut authorization = HeaderValue::from_str(&format!("token {}", token))
            .map_err(|_| GbError::Other("Invalid token header".into()))?;
        authorization.set_sensitive(true);
        headers.insert(AUTHORIZATION, authorization);
        headers.insert(ACCEPT, HeaderValue::from_static("application/json"));
        let client = crate::limits::http_builder()?
            .default_headers(headers)
            .redirect(reqwest::redirect::Policy::none())
            .build()?;
        let base_url = normalize_base_url(hostname, protocol)?;
        Ok(Self {
            client,
            base_url,
            secrets: Secrets::new(&[token]),
        })
    }

    pub async fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T> {
        self.request_json(Method::GET, path, None::<&()>, JsonContract::Standard)
            .await
    }

    pub async fn get_text_from_origin(&self, path_or_url: &str) -> Result<String> {
        let url = normalize_origin_path_or_url(path_or_url, &self.base_url)?;
        let (response, context) =
            transport::send(self.client.get(url), false, &self.secrets).await?;
        let body = transport::read(response, &context, &self.secrets).await?;
        if context.status.is_some_and(|s| (200..300).contains(&s)) {
            Ok(String::from_utf8_lossy(&body).into_owned())
        } else {
            Err(transport::status_error(&context, &body, &self.secrets))
        }
    }

    pub(crate) async fn get_with_headers<T: DeserializeOwned>(
        &self,
        path_or_url: &str,
    ) -> Result<(T, HeaderMap)> {
        let url = normalize_path_or_url(path_or_url, &self.base_url)?;
        let (response, context) =
            transport::send(self.client.get(url), false, &self.secrets).await?;
        let headers = response.headers().clone();
        let body = transport::read(response, &context, &self.secrets).await?;
        if !context.status.is_some_and(|s| (200..300).contains(&s)) {
            return Err(transport::status_error(&context, &body, &self.secrets));
        }
        let value = parse_success_body(
            &body,
            JsonContract::Standard,
            context.status.unwrap_or_default(),
        )
        .map_err(|err| context.error(err, "decode", &self.secrets))?;
        Ok((value, headers))
    }

    pub async fn post<T: DeserializeOwned, B: serde::Serialize>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<T> {
        self.request_json(Method::POST, path, Some(body), JsonContract::Standard)
            .await
    }
    pub async fn patch<T: DeserializeOwned, B: serde::Serialize>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<T> {
        self.request_json(Method::PATCH, path, Some(body), JsonContract::Standard)
            .await
    }
    pub async fn delete(&self, path: &str) -> Result<()> {
        let url = normalize_path_or_url(path, &self.base_url)?;
        let (response, context) =
            transport::send(self.client.delete(url), true, &self.secrets).await?;
        let body = transport::read(response, &context, &self.secrets).await?;
        if context.status.is_some_and(|s| (200..300).contains(&s)) {
            Ok(())
        } else {
            Err(transport::status_error(&context, &body, &self.secrets))
        }
    }

    pub async fn raw_request(
        &self,
        method: Method,
        path: &str,
        body: Option<&Value>,
    ) -> Result<Value> {
        let secrets = self.secrets.with_body(body)?;
        let url = normalize_path_or_url(path, &self.base_url)?;
        let mut request = self.client.request(method.clone(), url);
        if let Some(body) = body {
            request = request.json(body);
        }
        let mutating = !matches!(method, Method::GET | Method::HEAD | Method::OPTIONS);
        let (response, context) = transport::send(request, mutating, &secrets).await?;
        let body = transport::read(response, &context, &secrets).await?;
        if !context.status.is_some_and(|s| (200..300).contains(&s)) {
            return Err(transport::status_error(&context, &body, &secrets));
        }
        if body.is_empty() {
            return Ok(Value::Null);
        }
        parse_success_body(
            &body,
            JsonContract::Standard,
            context.status.unwrap_or_default(),
        )
        .map_err(|err| context.error(err, "decode", &secrets))
    }

    pub fn web_url(&self, path: &str) -> String {
        format!("{}{}", self.base_url.trim_end_matches("/api/v3"), path)
    }

    pub(crate) async fn request_json<T: DeserializeOwned, B: serde::Serialize>(
        &self,
        method: Method,
        path: &str,
        body: Option<&B>,
        contract: JsonContract,
    ) -> Result<T> {
        let secrets = self.secrets.with_body(body)?;
        let url = normalize_path_or_url(path, &self.base_url)?;
        let mut request = self.client.request(method.clone(), url);
        if let Some(body) = body {
            request = request.json(body);
        }
        let mutating = !matches!(method, Method::GET | Method::HEAD | Method::OPTIONS);
        let (response, context) = transport::send(request, mutating, &secrets).await?;
        let body = transport::read(response, &context, &secrets).await?;
        if !context.status.is_some_and(|s| (200..300).contains(&s)) {
            return Err(transport::status_error(&context, &body, &secrets));
        }
        parse_success_body(&body, contract, context.status.unwrap_or_default())
            .map_err(|err| context.error(err, "decode", &secrets))
    }
    pub(crate) fn base_url(&self) -> &str {
        &self.base_url
    }
}

fn parse_success_body<T: DeserializeOwned>(
    body: &[u8],
    contract: JsonContract,
    status: u16,
) -> Result<T> {
    let value: Value = serde_json::from_slice(body)?;

    let inner = match contract {
        JsonContract::Standard => None,
        // Confirmed 4.44 PR create returns a JSON-encoded object string.
        JsonContract::PullRequestCreate => value
            .as_str()
            .and_then(|text| serde_json::from_str::<Value>(text).ok())
            .filter(|v| v.is_object()),
        // Confirmed 4.44 merge serializes a Scalatra ActionResult. Require its
        // exact shape/status, not merely arbitrary status and body properties.
        JsonContract::PullRequestMerge => value
            .as_object()
            .filter(|v| {
                v.len() == 3
                    && v.get("status").and_then(Value::as_u64) == Some(u64::from(status))
                    && v.get("headers").is_some_and(Value::is_object)
            })
            .and_then(|v| v.get("body"))
            .and_then(Value::as_str)
            .and_then(|text| serde_json::from_str::<Value>(text).ok())
            .filter(|v| v.is_object()),
    };
    let value = inner.unwrap_or(value);
    if matches!(contract, JsonContract::PullRequestMerge)
        && !value.get("merged").is_some_and(Value::is_boolean)
    {
        return Err(GbError::Json(
            <serde_json::Error as serde::de::Error>::custom(
                "Expected a merge result with a boolean merged field",
            ),
        ));
    }
    Ok(serde_json::from_value(value)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};

    #[test]
    fn standard_typed_string_and_status_body_values_are_not_unwrapped() {
        let string: String =
            parse_success_body(br#""{\"literal\":true}""#, JsonContract::Standard, 200).unwrap();
        assert_eq!(string, r#"{"literal":true}"#);
        let object: Value = parse_success_body(
            br#"{"status":200,"body":"plain text","headers":{}}"#,
            JsonContract::Standard,
            200,
        )
        .unwrap();
        assert_eq!(object["body"], "plain text");
    }

    #[tokio::test]
    async fn text_contract_accepts_non_json_and_empty_204() {
        let _env = crate::test_support::TestEnv::new();
        for (status, body) in [
            ("200 OK", "diff --git a/file b/file\n"),
            ("204 No Content", ""),
        ] {
            let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            let port = listener.local_addr().unwrap().port();
            let server = std::thread::spawn(move || {
                let (mut stream, _) = listener.accept().unwrap();
                let mut header = Vec::new();
                while !header.ends_with(b"\r\n\r\n") {
                    let mut byte = [0];
                    stream.read_exact(&mut byte).unwrap();
                    header.push(byte[0]);
                }
                stream.write_all(format!("HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).as_bytes()).unwrap();
            });
            let client = ApiClient::new(&format!("127.0.0.1:{port}"), "token", "http").unwrap();
            assert_eq!(
                client
                    .get_text_from_origin("/alice/project.diff")
                    .await
                    .unwrap(),
                body
            );
            server.join().unwrap();
        }
    }
}
