//! Immutable comparison identities; branch names are only object-fetch hints.
use crate::error::{GbError, Result};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CommitId(String);

impl CommitId {
    pub fn parse(value: &str) -> Result<Self> {
        if !matches!(value.len(), 40 | 64)
            || !value.bytes().all(|byte| byte.is_ascii_hexdigit())
            || value.bytes().all(|byte| byte == b'0')
        {
            return Err(GbError::Other(
                "Expected a complete, nonzero saved commit SHA (40 or 64 hexadecimal digits)."
                    .into(),
            ));
        }
        Ok(Self(value.to_ascii_lowercase()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

pub(crate) struct PullRequestSnapshot {
    pub number: u64,
    pub base: CommitId,
    pub head: CommitId,
    pub base_branch: Option<String>,
    pub head_branch: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requires_full_commit_ids_instead_of_git_revision_expressions() {
        for value in [
            "main",
            "HEAD",
            "--help",
            "abcd",
            "HEAD^{commit}",
            &"0".repeat(40),
            &"g".repeat(40),
        ] {
            assert!(CommitId::parse(value).is_err());
        }
        for length in [40, 64] {
            assert_eq!(
                CommitId::parse(&"A".repeat(length)).unwrap().as_str(),
                "a".repeat(length)
            );
        }
    }
}
