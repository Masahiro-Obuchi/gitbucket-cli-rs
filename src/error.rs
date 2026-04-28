use thiserror::Error;

#[derive(Error, Debug)]
pub enum GbError {
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
            GbError::DiffUnavailable { cause, .. } => Some(cause),
            _ => None,
        }
    }

    pub fn status(&self) -> Option<u16> {
        match self {
            GbError::Api { status, .. } => Some(*status),
            _ => None,
        }
    }
}

pub type Result<T> = std::result::Result<T, GbError>;
