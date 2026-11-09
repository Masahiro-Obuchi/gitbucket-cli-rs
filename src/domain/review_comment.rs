//! Discussion IDs and CommitComment IDs belong to different storage namespaces.
use crate::error::{GbError, Result};
use serde::Serialize;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum CommentSelection {
    #[default]
    Discussion,
    Review,
    All,
}
impl std::str::FromStr for CommentSelection {
    type Err = GbError;
    fn from_str(value: &str) -> Result<Self> {
        match value {
            "discussion" => Ok(Self::Discussion),
            "review" => Ok(Self::Review),
            "all" => Ok(Self::All),
            _ => Err(GbError::Other(
                "Comment kind must be discussion, review or all.".into(),
            )),
        }
    }
}
impl CommentSelection {
    pub fn discussion(self) -> bool {
        self != Self::Review
    }
    pub fn review(self) -> bool {
        self != Self::Discussion
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum CommentKind {
    Discussion,
    Inline,
    Commit,
}
impl std::fmt::Display for CommentKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Discussion => "discussion",
            Self::Inline => "inline",
            Self::Commit => "commit",
        })
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub(crate) struct CommentIdentity {
    pub instance: String,
    pub kind: CommentKind,
    pub repository: String,
    pub id: u64,
}
impl std::fmt::Display for CommentIdentity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}:{}/{}:{}",
            self.kind, self.instance, self.repository, self.id
        )
    }
}
#[derive(Clone, Debug, Serialize)]
pub(crate) struct RenderedBody {
    pub format: &'static str,
    pub html: String,
    pub text: String,
    pub markdown: Option<String>,
}
#[derive(Clone, Debug, Serialize)]
pub(crate) struct ReviewPosition {
    pub source: &'static str,
    pub validated_against_git: bool,
    pub path: Option<String>,
    pub old_line: Option<u64>,
    pub new_line: Option<u64>,
    pub commit_id: Option<String>,
    pub original_commit_id: Option<String>,
    pub original_old_line: Option<u64>,
    pub original_new_line: Option<u64>,
    pub outdated: Option<bool>,
}
#[derive(Clone, Debug, Serialize)]
pub(crate) struct ReviewThread {
    pub id: String,
    pub relation: &'static str,
    pub in_reply_to: Option<CommentIdentity>,
}
#[derive(Clone, Debug, Serialize)]
pub(crate) struct ReviewComment {
    pub identity: CommentIdentity,
    pub visible_on_pull_request: u64,
    pub pull_request: Option<u64>,
    pub author: String,
    pub body: RenderedBody,
    pub created_at: String,
    pub timestamp_format: &'static str,
    pub timezone: Option<String>,
    pub updated_at: Option<String>,
    pub html_url: String,
    pub position: ReviewPosition,
    pub thread: Option<ReviewThread>,
}
