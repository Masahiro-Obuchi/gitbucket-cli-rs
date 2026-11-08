//! Explicit CLI augmentation of immutable API JSON; never replace Issue counts.
use serde_json::{Map, Value};

use crate::error::{GbError, Result};
use crate::models::{comment::Comment, issue::Issue, pull_request::PullRequest};

fn insert_details(object: &mut Map<String, Value>, name: &str, value: Value) -> Result<()> {
    if object.contains_key(name) {
        return Err(GbError::Other(format!(
            "API response already contains {name}; cannot add CLI comment details without losing server data. Use view without --comments or gb api."
        )));
    }
    object.insert(name.into(), value);
    Ok(())
}

pub(crate) fn issue(issue: &Issue, comments: &[Comment]) -> Result<Value> {
    let mut value = serde_json::to_value(issue)?;
    let object = value
        .as_object_mut()
        .ok_or_else(|| GbError::Other("Expected Issue JSON object".into()))?;
    insert_details(object, "comment_details", serde_json::to_value(comments)?)?;
    Ok(value)
}

pub(crate) fn pull_request(pr: &PullRequest, comments: &[Comment]) -> Result<Value> {
    let mut value = serde_json::to_value(pr)?;
    let object = value
        .as_object_mut()
        .ok_or_else(|| GbError::Other("Expected PR JSON object".into()))?;
    if let Some(original) = object.get("comments").cloned() {
        insert_details(object, "api_comments", original)?;
    }
    // Preserve the PR CLI's established comments array. Inline/commit review
    // comments will have a distinct type/array when their reader is implemented.
    object.insert("comments".into(), serde_json::to_value(comments)?);
    Ok(value)
}
