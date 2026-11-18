//! Explicit CLI augmentation of immutable API JSON; never replace Issue counts.
use serde_json::{Map, Value};

use crate::error::{GbError, Result};
use crate::models::comment::Comment;

fn insert_details(object: &mut Map<String, Value>, name: &str, value: Value) -> Result<()> {
    if object.contains_key(name) {
        return Err(GbError::Other(format!(
            "API response already contains {name}; cannot add CLI comment details without losing server data. Use view without --comments or gb api."
        )));
    }
    object.insert(name.into(), value);
    Ok(())
}

pub(crate) fn add_discussion(value: &mut Value, comments: &[Comment]) -> Result<()> {
    let object = value
        .as_object_mut()
        .ok_or_else(|| GbError::Other("Expected Issue or PR JSON object".into()))?;
    insert_details(object, "comment_details", serde_json::to_value(comments)?)
}

pub(crate) fn add_reviews(
    value: &mut Value,
    comments: &crate::application::pr_comments::PrComments,
) -> Result<()> {
    let object = value
        .as_object_mut()
        .ok_or_else(|| GbError::Other("Expected PR JSON object".into()))?;
    insert_details(
        object,
        "review_comments",
        serde_json::to_value(&comments.review_comments)?,
    )?;
    insert_details(
        object,
        "comment_coverage",
        serde_json::to_value(&comments.comment_coverage)?,
    )?;
    Ok(())
}
