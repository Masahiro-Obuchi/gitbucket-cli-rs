use crate::cli::common::RepoContext;
use crate::error::{GbError, Result};
use crate::models::milestone::{Milestone, UpdateMilestone};
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

pub(super) fn validate_title(title: &str) -> Result<()> {
    if title.trim().is_empty() || title.encode_utf16().count() > 100 {
        return Err(GbError::Other("Milestone title must be nonempty and at most 100 UTF-16 code units (GitBucket's limit).".into()));
    }
    Ok(())
}

/// The pinned server's CreateAMilestone.isValid restriction, not a Web restriction.
pub(super) fn rest_title(title: &str) -> bool {
    !title.is_empty()
        && title.len() <= 100
        && title
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'+' | b'_' | b'.'))
}

pub(super) fn validate_web_text(title: &str, description: Option<&str>) -> Result<()> {
    validate_title(title)?;
    // Scalatra's trim wrapper calls Java String.trim: only code units through
    // U+0020 are removed. Rust's Unicode whitespace rule has different boundaries.
    let trims = |value: &str| value.trim_matches(|ch| ch <= '\u{20}') != value;
    if trims(title) || description.is_some_and(trims) {
        return Err(GbError::Other("GitBucket's milestone Web form trims title/description whitespace and cannot preserve these values. Use a REST-compatible title or explicitly provide values without surrounding whitespace.".into()));
    }
    Ok(())
}

fn same_due(left: Option<&str>, right: Option<&str>) -> bool {
    fn meaningful(value: Option<&str>) -> Option<&str> {
        value.filter(|value| !value.starts_with("0001-01-01"))
    }
    match (meaningful(left), meaningful(right)) {
        (None, None) => true,
        (Some(left), Some(right)) if left == right => true,
        (Some(left), Some(right)) => match (
            OffsetDateTime::parse(left, &Rfc3339),
            OffsetDateTime::parse(right, &Rfc3339),
        ) {
            (Ok(left), Ok(right)) => left == right,
            _ => false,
        },
        _ => false,
    }
}

pub(super) fn verify_web_result(actual: &Milestone, expected: &UpdateMilestone) -> Result<()> {
    if actual.title != expected.title
        || actual.state != expected.state
        || actual.description.as_deref().unwrap_or_default()
            != expected.description.as_deref().unwrap_or_default()
        || !same_due(actual.due_on.as_deref(), expected.due_on.as_deref())
    {
        return Err(GbError::Other("Milestone Web mutation was submitted, but the stored fields do not match the requested values. Inspect the milestone before retrying; the operation may be partially applied.".into()));
    }
    Ok(())
}

pub(super) async fn read_after_web(ctx: &RepoContext, number: u64) -> Result<Milestone> {
    ctx.client.get_milestone(&ctx.owner, &ctx.repo, number).await.map_err(|err|
        GbError::Other(format!("Milestone Web mutation was submitted, but its result could not be verified: {err}. Inspect the milestone before retrying.")))
}
