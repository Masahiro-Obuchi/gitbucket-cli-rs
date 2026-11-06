//! Shared deadlines, response reads and bounded diagnostics. Never retry writes.
use reqwest::{RequestBuilder, Response};

use crate::error::{GbError, Result};

const ERROR_BODY_LIMIT: usize = 8192;
const DETAIL_LIMIT: usize = 2048;

#[derive(Clone, Default)]
pub(crate) struct Secrets(Vec<String>);

impl std::fmt::Debug for Secrets {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("[REDACTED]")
    }
}

impl Secrets {
    pub fn with_body<B: serde::Serialize>(&self, body: Option<&B>) -> Result<Self> {
        fn collect(value: &serde_json::Value, sensitive: bool, values: &mut Vec<String>) {
            match value {
                serde_json::Value::String(value) if sensitive && !value.is_empty() => {
                    values.push(value.clone())
                }
                serde_json::Value::Array(items) => {
                    for item in items {
                        collect(item, sensitive, values);
                    }
                }
                serde_json::Value::Object(items) => {
                    for (key, value) in items {
                        let key = key.to_ascii_lowercase();
                        let sensitive = sensitive
                            || [
                                "token",
                                "password",
                                "secret",
                                "authorization",
                                "cookie",
                                "credential",
                            ]
                            .iter()
                            .any(|part| key.contains(part));
                        collect(value, sensitive, values);
                    }
                }
                _ => {}
            }
        }
        let mut secrets = self.clone();
        if let Some(body) = body {
            collect(&serde_json::to_value(body)?, false, &mut secrets.0);
        }
        secrets
            .0
            .sort_by_key(|value| std::cmp::Reverse(value.len()));
        Ok(secrets)
    }
    pub fn new(values: &[&str]) -> Self {
        let mut secrets: Vec<String> = values
            .iter()
            .filter(|value| !value.is_empty())
            .map(|value| (*value).into())
            .collect();
        for name in ["GB_TOKEN", "GB_PASSWORD"] {
            if let Ok(value) = std::env::var(name) {
                if !value.is_empty() {
                    secrets.push(value);
                }
            }
        }
        secrets.sort_by_key(|value| std::cmp::Reverse(value.len()));
        Self(secrets)
    }

    pub fn clean(&self, text: &str) -> String {
        let mut text = text.to_owned();
        for secret in &self.0 {
            for value in [
                secret.clone(),
                url::form_urlencoded::byte_serialize(secret.as_bytes()).collect::<String>(),
                percent_encoding::utf8_percent_encode(secret, percent_encoding::NON_ALPHANUMERIC)
                    .to_string(),
                serde_json::to_string(secret)
                    .unwrap_or_default()
                    .trim_matches('"')
                    .to_string(),
            ] {
                if !value.is_empty() {
                    text = text.replace(&value, "[REDACTED]");
                }
            }
        }
        let text: String = text
            .chars()
            .map(|ch| {
                if ch.is_control() && ch != '\n' && ch != '\t' {
                    '?'
                } else {
                    ch
                }
            })
            .collect();
        truncate(&text, DETAIL_LIMIT)
    }
}

#[derive(Clone)]
pub(crate) struct Context {
    operation: String,
    pub status: Option<u16>,
    mutating: bool,
    html: bool,
}

impl Context {
    pub fn error(&self, source: GbError, phase: &'static str, secrets: &Secrets) -> GbError {
        let detail = secrets.clean(&source.to_string());
        GbError::Request {
            source: Box::new(source),
            operation: self.operation.clone(),
            phase,
            status: self.status,
            mutating: self.mutating,
            detail,
        }
    }
}

pub(crate) async fn send(
    builder: RequestBuilder,
    mutating: bool,
    secrets: &Secrets,
) -> Result<(Response, Context)> {
    let (client, request) = builder.build_split();
    let request = request.map_err(|error| GbError::Http(error.without_url()))?;
    // URLs, queries, userinfo, request headers and form bodies are not diagnostics.
    let context = Context {
        operation: secrets.clean(&format!("{} {}", request.method(), request.url().path())),
        status: None,
        mutating,
        html: false,
    };
    let response = tokio::select! {
        biased;
        _ = crate::cancel::notified() => return Err(context.error(GbError::Cancelled, "send", secrets)),
        result = client.execute(request) => result.map_err(|error| context.error(GbError::Http(error.without_url()), "send", secrets))?,
    };
    let context = Context {
        status: Some(response.status().as_u16()),
        html: response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .is_some_and(|value| value.to_ascii_lowercase().starts_with("text/html")),
        ..context
    };
    Ok((response, context))
}

/// Read success bodies completely, including empty-response operations. A
/// truncated error body is bounded, while a broken transfer keeps its cause.
pub(crate) async fn read(
    mut response: Response,
    context: &Context,
    secrets: &Secrets,
) -> Result<Vec<u8>> {
    let mut body = Vec::new();
    let limit = (!response.status().is_success()).then_some(ERROR_BODY_LIMIT);
    loop {
        let chunk = tokio::select! {
            biased;
            _ = crate::cancel::notified() => return Err(context.error(GbError::Cancelled, "body", secrets)),
            result = response.chunk() => result.map_err(|error| context.error(GbError::Http(error.without_url()), "body", secrets))?,
        };
        let Some(chunk) = chunk else {
            break;
        };
        let remaining = limit.map_or(chunk.len(), |limit| limit.saturating_sub(body.len()));
        body.extend_from_slice(&chunk[..chunk.len().min(remaining)]);
        if limit.is_some_and(|limit| body.len() >= limit) {
            break;
        }
    }
    Ok(body)
}

pub(crate) fn status_error(context: &Context, body: &[u8], secrets: &Secrets) -> GbError {
    let text = String::from_utf8_lossy(body);
    let detail = if context.html || text.trim_start().starts_with('<') {
        "HTML error response omitted".into()
    } else {
        let value = serde_json::from_str::<serde_json::Value>(&text).ok();
        let text = value
            .as_ref()
            .and_then(|value| value.get("message"))
            .and_then(|value| value.as_str())
            .unwrap_or(&text);
        // Do not echo structured credentials, cookies, auth headers or URLs from
        // proxy/server diagnostics. Known configured secrets are redacted too.
        let safe = text
            .lines()
            .map(|line| {
                let lower = line.to_ascii_lowercase();
                if [
                    "token",
                    "password",
                    "secret",
                    "authorization",
                    "cookie",
                    "session",
                    "credential",
                    "://",
                ]
                .iter()
                .any(|key| lower.contains(key))
                {
                    "[sensitive response detail omitted]"
                } else {
                    line
                }
            })
            .collect::<Vec<_>>()
            .join(" ");
        secrets.clean(&safe)
    };
    let status = context.status.unwrap_or_default();
    let source = GbError::Api {
        status,
        message: format!(
            "{}: {}{}",
            context.operation,
            detail,
            if body.len() >= ERROR_BODY_LIMIT {
                " [truncated]"
            } else {
                ""
            }
        ),
    };
    if context.mutating && !(400..500).contains(&status) {
        context.error(source, "status", secrets)
    } else {
        source
    }
}

fn truncate(value: &str, limit: usize) -> String {
    if value.len() <= limit {
        return value.into();
    }
    let mut end = limit;
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    format!("{} [truncated]", &value[..end])
}
