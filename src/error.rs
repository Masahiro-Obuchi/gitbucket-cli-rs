use thiserror::Error;

#[derive(Error, Debug)]
pub enum GbError {
    #[error("git {operation} {reason}{}{}", .source.as_ref().map(|error| format!(": {error}")).unwrap_or_default(), if *.mutating { ". Inspect local Git state before retrying; local changes may already be applied." } else { "" })]
    Git {
        operation: &'static str,
        reason: &'static str,
        source: Option<std::io::Error>,
        mutating: bool,
    },
    #[error("Operation cancelled")]
    Cancelled,
    #[error("{operation}: {detail} ({phase}){}", if *.mutating { ". Outcome is unverified; inspect the target before retrying. No automatic retry was attempted." } else { "" })]
    Request {
        source: Box<GbError>,
        operation: String,
        phase: &'static str,
        status: Option<u16>,
        mutating: bool,
        detail: String,
    },
    #[error("{source}")]
    Unverified { source: Box<GbError> },
    #[error("{source}. Operations: {}. Inspect the target before retrying; unverified requests may already be applied.", crate::operation::describe(.operations))]
    Operation {
        source: Box<GbError>,
        operations: Vec<crate::operation::Operation>,
    },
    #[error("Authentication error: {0}")]
    Auth(String),

    #[error("API error ({status}): {message}")]
    Api { status: u16, message: String },

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Not authenticated. Run `gb auth login` first.")]
    NotAuthenticated,

    #[error("Repository not found. Specify with --repo OWNER/REPO or run from a git repository.")]
    RepoNotFound,

    #[error("Diff unavailable for pull request #{number}: {message}")]
    DiffUnavailable {
        number: u64,
        cause: &'static str,
        message: String,
    },

    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("TOML serialization error: {0}")]
    TomlSer(#[from] toml::ser::Error),

    #[error("TOML deserialization error: {0}")]
    TomlDe(#[from] toml::de::Error),

    #[error("URL parse error: {0}")]
    UrlParse(#[from] url::ParseError),

    #[error("Prompt error: {0}")]
    Dialoguer(#[from] dialoguer::Error),

    #[error("{0}")]
    Other(String),
}

impl GbError {
    pub fn code(&self) -> &'static str {
        match self {
            GbError::Git {
                reason: "timed out",
                ..
            } => "git_timeout",
            GbError::Git {
                reason: "cancelled",
                ..
            } => "cancelled",
            GbError::Git { .. } => "io_error",
            GbError::Cancelled => "cancelled",
            GbError::Request { source, .. } => source.code(),
            GbError::Operation { source, .. } | GbError::Unverified { source } => source.code(),
            GbError::Auth(_) => "auth_error",
            GbError::Api { .. } => "api_error",
            GbError::Config(_) => "config_error",
            GbError::NotAuthenticated => "not_authenticated",
            GbError::RepoNotFound => "repo_not_found",
            GbError::DiffUnavailable { .. } => "diff_unavailable",
            GbError::Http(_) => "http_error",
            GbError::Io(_) => "io_error",
            GbError::Json(_) => "json_error",
            GbError::TomlSer(_) => "toml_serialization_error",
            GbError::TomlDe(_) => "toml_deserialization_error",
            GbError::UrlParse(_) => "url_parse_error",
            GbError::Dialoguer(_) => "prompt_error",
            GbError::Other(_) => "error",
        }
    }

    pub fn cause_code(&self) -> Option<&'static str> {
        match self {
            GbError::Cancelled => Some("cancelled"),
            GbError::Git {
                reason: "timed out",
                ..
            } => Some("timeout"),
            GbError::Git {
                reason: "cancelled",
                ..
            } => Some("cancelled"),
            GbError::Request { source, phase, .. } => Some(match source.as_ref() {
                GbError::Http(error) if error.is_timeout() => "timeout",
                GbError::Http(error) if error.is_connect() => "connect_failed",
                GbError::Cancelled => "cancelled",
                GbError::Json(_) => "invalid_json",
                _ if *phase == "body" => "body_read_failed",
                _ => "request_failed",
            }),
            GbError::Operation { source, .. } | GbError::Unverified { source } => {
                source.cause_code()
            }
            GbError::DiffUnavailable { cause, .. } => Some(cause),
            _ => None,
        }
    }

    pub fn status(&self) -> Option<u16> {
        match self {
            GbError::Request { status, .. } => *status,
            GbError::Operation { source, .. } | GbError::Unverified { source } => source.status(),
            GbError::Api { status, .. } => Some(*status),
            _ => None,
        }
    }

    pub fn operations(&self) -> Option<&[crate::operation::Operation]> {
        match self {
            Self::Operation { operations, .. } => Some(operations),
            _ => None,
        }
    }

    pub fn outcome(&self) -> Option<&'static str> {
        match self {
            Self::Request { mutating: true, .. } | Self::Unverified { .. } => Some("unverified"),
            Self::Git {
                mutating: true,
                reason: "timed out" | "cancelled" | "output read failed",
                ..
            } => Some("unverified"),
            Self::Operation { source, operations } => {
                if operations
                    .iter()
                    .any(|operation| operation.status == crate::operation::Status::Unverified)
                {
                    Some("unverified")
                } else {
                    source.outcome()
                }
            }
            _ => None,
        }
    }
}

pub type Result<T> = std::result::Result<T, GbError>;
