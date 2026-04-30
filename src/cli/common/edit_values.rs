use crate::error::{GbError, Result};

/// Normalize the state filter for list commands.
pub fn normalize_list_state(state: &str) -> Result<String> {
    match state.to_ascii_lowercase().as_str() {
        "open" | "closed" | "all" => Ok(state.to_ascii_lowercase()),
        _ => Err(GbError::Other(format!(
            "Invalid state '{}'. Expected one of: open, closed, all",
            state
        ))),
    }
}

/// Normalize an issue-like edit state.
pub fn normalize_edit_state(kind: &str, state: Option<String>) -> Result<Option<String>> {
    match state {
        None => Ok(None),
        Some(value) => match value.to_ascii_lowercase().as_str() {
            "open" | "closed" => Ok(Some(value.to_ascii_lowercase())),
            _ => Err(GbError::Other(format!(
                "Invalid {} state. Expected 'open' or 'closed'.",
                kind
            ))),
        },
    }
}

/// Normalize repeated string arguments.
pub fn normalize_str_vec(values: Vec<String>) -> Vec<String> {
    values
        .into_iter()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
        .collect()
}

/// Apply remove operations first, then append missing additions.
pub fn merge_named_values(
    current: impl IntoIterator<Item = String>,
    additions: Vec<String>,
    removals: Vec<String>,
) -> Vec<String> {
    let mut values: Vec<String> = current.into_iter().collect();
    values.retain(|value| !removals.iter().any(|removed| removed == value));
    for addition in additions {
        if !values.iter().any(|existing| existing == &addition) {
            values.push(addition);
        }
    }
    values
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_list_state_accepts_supported_values() {
        assert_eq!(normalize_list_state("OPEN").unwrap(), "open");
        assert_eq!(normalize_list_state("closed").unwrap(), "closed");
        assert_eq!(normalize_list_state("all").unwrap(), "all");
        assert!(normalize_list_state("draft").is_err());
    }

    #[test]
    fn normalize_edit_state_accepts_open_and_closed() {
        assert_eq!(
            normalize_edit_state("issue", Some("open".into())).unwrap(),
            Some("open".into())
        );
        assert_eq!(
            normalize_edit_state("issue", Some("Closed".into())).unwrap(),
            Some("closed".into())
        );
    }

    #[test]
    fn normalize_edit_state_rejects_other_values() {
        assert!(normalize_edit_state("issue", Some("all".into())).is_err());
    }

    #[test]
    fn merge_named_values_applies_removals_then_additions() {
        let merged = merge_named_values(
            vec!["bug".into(), "urgent".into()],
            vec!["enhancement".into(), "urgent".into()],
            vec!["bug".into()],
        );

        assert_eq!(merged, vec!["urgent", "enhancement"]);
    }

    #[test]
    fn normalize_str_vec_trims_whitespace_and_drops_empty() {
        assert_eq!(
            normalize_str_vec(vec!["bug".into(), " urgent".into(), "".into()]),
            vec!["bug", "urgent"]
        );
        assert_eq!(
            normalize_str_vec(vec!["  alice  ".into(), "  ".into(), "bob".into()]),
            vec!["alice", "bob"]
        );
        assert_eq!(
            normalize_str_vec(vec!["".into(), "  ".into()]),
            Vec::<String>::new()
        );
    }
}
