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

pub type Result<T> = std::result::Result<T, GbError>;
