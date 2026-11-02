/// User intent for nullable milestone fields, before merging with stored values.
pub(super) enum Change<T> {
    Unchanged,
    Set(T),
    Clear,
}

impl Change<String> {
    pub(super) fn text(value: Option<String>) -> Self {
        match value {
            None => Self::Unchanged,
            Some(value) if value.is_empty() => Self::Clear,
            Some(value) => Self::Set(value),
        }
    }

    pub(super) fn merge(&self, current: &Option<String>) -> Option<String> {
        match self {
            Self::Unchanged => current.clone(),
            Self::Set(value) => Some(value.clone()),
            Self::Clear => None,
        }
    }
}
