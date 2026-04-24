use time::format_description::well_known::Rfc3339;
use time::macros::format_description;
use time::{Date, OffsetDateTime};

use crate::error::{GbError, Result};

pub(super) struct NormalizedDueOn {
    pub(super) api_value: String,
    pub(super) form_value: String,
}

pub(super) enum DueOnInput {
    Unchanged,
    Clear,
    Set(NormalizedDueOn),
}

pub(super) fn normalize_due_on_for_create(raw: Option<String>) -> Result<Option<NormalizedDueOn>> {
    match raw {
        Some(value) if value.trim().is_empty() => Ok(None),
        Some(value) => Ok(Some(parse_due_on_value(&value)?)),
        None => Ok(None),
    }
}

pub(super) fn normalize_due_on_for_edit(raw: Option<String>) -> Result<DueOnInput> {
    match raw {
        Some(value) if value.trim().is_empty() => Ok(DueOnInput::Clear),
        Some(value) => Ok(DueOnInput::Set(parse_due_on_value(&value)?)),
        None => Ok(DueOnInput::Unchanged),
    }
}

fn parse_due_on_value(value: &str) -> Result<NormalizedDueOn> {
    static DATE_FORMAT: &[time::format_description::FormatItem<'static>] =
        format_description!("[year]-[month]-[day]");

    let trimmed = value.trim();

    if let Ok(date) = Date::parse(trimmed, DATE_FORMAT) {
        let form_value = date
            .format(DATE_FORMAT)
            .map_err(|err| GbError::Other(format!("Failed to format due date: {}", err)))?;
        return Ok(NormalizedDueOn {
            api_value: format!("{form_value}T00:00:00Z"),
            form_value,
        });
    }

    if let Ok(date_time) = OffsetDateTime::parse(trimmed, &Rfc3339) {
        let form_value = date_time
            .date()
            .format(DATE_FORMAT)
            .map_err(|err| GbError::Other(format!("Failed to format due date: {}", err)))?;
        return Ok(NormalizedDueOn {
            api_value: format!("{form_value}T00:00:00Z"),
            form_value,
        });
    }

    Err(GbError::Other(format!(
        "Invalid due date '{}'. Expected YYYY-MM-DD or RFC3339.",
        value
    )))
}

pub(super) fn due_on_to_form_date(value: &str) -> Result<String> {
    if value.starts_with("0001-01-01") {
        return Ok(String::new());
    }
    Ok(parse_due_on_value(value)?.form_value)
}

pub(super) fn format_due_on(value: Option<&str>) -> String {
    match value {
        Some(value) if value.starts_with("0001-01-01") => String::new(),
        Some(value) => value.to_string(),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::{normalize_due_on_for_create, normalize_due_on_for_edit, DueOnInput};

    #[test]
    fn create_due_on_accepts_plain_date() {
        let due_on = normalize_due_on_for_create(Some("2026-04-01".into())).unwrap();
        let due_on = due_on.unwrap();
        assert_eq!(due_on.api_value, "2026-04-01T00:00:00Z");
        assert_eq!(due_on.form_value, "2026-04-01");
    }

    #[test]
    fn create_due_on_accepts_rfc3339() {
        let due_on = normalize_due_on_for_create(Some("2026-04-01T09:30:00Z".into())).unwrap();
        let due_on = due_on.unwrap();
        assert_eq!(due_on.api_value, "2026-04-01T00:00:00Z");
        assert_eq!(due_on.form_value, "2026-04-01");
    }

    #[test]
    fn edit_due_on_empty_string_clears_value() {
        assert!(matches!(
            normalize_due_on_for_edit(Some(String::new())).unwrap(),
            DueOnInput::Clear
        ));
    }

    #[test]
    fn due_on_rejects_invalid_values() {
        assert!(normalize_due_on_for_create(Some("not-a-date".into())).is_err());
    }

    #[test]
    fn format_due_on_hides_unset_sentinel() {
        assert_eq!(super::format_due_on(Some("0001-01-01T00:00:00Z")), "");
        assert_eq!(
            super::format_due_on(Some("2026-04-01T00:00:00Z")),
            "2026-04-01T00:00:00Z"
        );
    }

    #[test]
    fn due_on_to_form_date_hides_unset_sentinel() {
        assert_eq!(
            super::due_on_to_form_date("0001-01-01T00:00:00Z").unwrap(),
            ""
        );
        assert_eq!(
            super::due_on_to_form_date("2026-04-01T00:00:00Z").unwrap(),
            "2026-04-01"
        );
    }
}
