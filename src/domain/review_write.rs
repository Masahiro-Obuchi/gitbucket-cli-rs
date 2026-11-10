//! Validated GitBucket coordinates, distinct from unverified Web display data.
use crate::error::{GbError, Result};
use serde::Serialize;

#[derive(Clone, Copy, Debug)]
pub(crate) enum Side {
    Old,
    New,
}

impl std::str::FromStr for Side {
    type Err = GbError;

    fn from_str(value: &str) -> Result<Self> {
        match value {
            "old" => Ok(Self::Old),
            "new" => Ok(Self::New),
            _ => Err(invalid("Review side must be old or new.")),
        }
    }
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub(crate) struct InlinePosition {
    pub path: String,
    pub old_line: Option<u64>,
    pub new_line: Option<u64>,
}

pub(crate) struct InlineSubmission<'a> {
    pub number: u64,
    pub head: &'a super::pull_request::CommitId,
    pub position: &'a InlinePosition,
    pub body: &'a str,
    pub diff: &'a str,
}

pub(crate) fn invalid(message: impl Into<String>) -> GbError {
    GbError::ReviewWrite {
        cause: "invalid_write_target",
        message: message.into(),
    }
}

pub(crate) fn normalized_body(body: &str) -> Result<String> {
    let body = body.replace("\r\n", "\n").replace('\r', "\n");
    let body = body.trim_matches(|c| c <= '\u{20}');
    if body.is_empty() {
        return Err(invalid("A nonempty review body is required."));
    }
    Ok(body.into())
}

pub(crate) fn validate_path(path: &str) -> Result<()> {
    if path.is_empty()
        || path.starts_with('/')
        || path.contains('\\')
        || path.chars().any(char::is_control)
        || path.trim_matches(|c| c <= '\u{20}') != path
        || path.split('/').any(|part| matches!(part, "" | "." | ".."))
    {
        return Err(invalid("Use an exact repository-relative UTF-8 path. Paths with control characters, backslashes or boundary whitespace cannot round-trip through GitBucket's comment form."));
    }
    Ok(())
}
