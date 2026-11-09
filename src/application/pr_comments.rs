//! One reader and ordering contract for PR view and comment list.
use crate::{
    api::client::ApiClient,
    domain::{pull_request::CommitId, review_comment::*},
    error::{GbError, Result},
    models::{comment::Comment, pull_request::PullRequest},
};
use serde::Serialize;

pub(crate) async fn review_target(
    client: &ApiClient,
    owner: &str,
    repo: &str,
    number: u64,
) -> Result<(PullRequest, CommitId)> {
    // Same narrow 200/empty contract verified for deleted forks by PR-10.
    let (pr, canonical) = match client
        .pull_request_snapshot_details(owner, repo, number)
        .await?
    {
        Some(pr) => (pr, true),
        None => (
            client
                .list_pull_requests(owner, repo, "all")
                .await?
                .into_iter()
                .find(|pr| pr.number == number)
                .ok_or_else(|| GbError::Other(format!("Pull request #{number} not found")))?,
            false,
        ),
    };
    if pr.number != number {
        return Err(GbError::Other(
            "PR target mismatch while reading review comments.".into(),
        ));
    }
    if pr
        .base
        .as_ref()
        .and_then(|base| base.repo.as_ref())
        .is_some_and(|base| base.full_name != format!("{owner}/{repo}"))
    {
        return Err(GbError::Other(
            "PR base repository does not match the selected review target.".into(),
        ));
    }
    let head = pr
        .head
        .as_ref()
        .and_then(|head| head.sha.as_deref())
        .and_then(|sha| CommitId::parse(sha).ok())
        .ok_or_else(|| {
            crate::api::web::review::unavailable(
                "missing_head_identity",
                "A saved PR head SHA is required to verify Web comment positions.",
            )
        })?;
    let pr = if canonical {
        pr
    } else {
        pr.with_field("head", serde_json::Value::Null)?
    };
    Ok((pr, head))
}

#[derive(Serialize)]
pub(crate) struct SourceCoverage {
    pub source_contract: &'static str,
    pub status: &'static str,
    pub scope: &'static str,
    pub kind: &'static str,
    pub reason: Option<&'static str>,
}
#[derive(Serialize)]
pub(crate) struct CommentCoverage {
    pub instance: String,
    pub requested: CommentSelection,
    pub repository: String,
    pub discussion: SourceCoverage,
    pub review: SourceCoverage,
    pub historical_review_complete: Option<bool>,
    pub historical_review_limit: &'static str,
    pub web_actor: Option<String>,
    pub review_access: &'static str,
    pub head_commit_id: Option<String>,
}
#[derive(Serialize)]
pub(crate) struct PrComments {
    pub comments: Vec<Comment>,
    pub review_comments: Vec<ReviewComment>,
    pub comment_coverage: CommentCoverage,
}

pub(crate) struct ReviewRead {
    pub comments: Vec<ReviewComment>,
    pub web_actor: Option<String>,
}

pub(crate) async fn read(
    client: &ApiClient,
    reviews: Option<ReviewRead>,
    owner: &str,
    repo: &str,
    number: u64,
    selection: CommentSelection,
    head: Option<&CommitId>,
) -> Result<PrComments> {
    let comments = if selection.discussion() {
        client.list_pr_comments(owner, repo, number).await?
    } else {
        Vec::new()
    };
    let (review_comments, web_actor) = if selection.review() {
        let (Some(reviews), Some(head)) = (reviews, head) else {
            return Err(crate::api::web::review::unavailable(
                "missing_review_source",
                "Review comments require a readable Web source and saved PR identity.",
            ));
        };
        let (_, after) = review_target(client, owner, repo, number).await?;
        if after != *head {
            return Err(crate::api::web::review::unavailable("head_changed","The PR head changed while reading its Web conversation. Retry to obtain consistent comment positions."));
        }
        (reviews.comments, reviews.web_actor)
    } else {
        (Vec::new(), None)
    };
    let review_access = if !selection.review() {
        "not_requested"
    } else if web_actor.is_some() {
        "cookie_session"
    } else {
        "token_or_public"
    };
    Ok(PrComments {
        comments,
        review_comments,
        comment_coverage: CommentCoverage {
            instance: client.web_url(""),
            requested: selection,
            repository: format!("{owner}/{repo}"),
            discussion: SourceCoverage {
                source_contract: "issue_discussion_rest",
                status: if selection.discussion() { "complete" } else { "not_requested" },
                scope: "issue_discussion_api",
                kind: "discussion",
                reason: (!selection.discussion()).then_some("Filtered out by --kind review."),
            },
            review: SourceCoverage {
                source_contract: "gitbucket_4_44_conversation_html",
                status: if selection.review() { "complete_for_scope" } else { "not_requested" },
                scope: "visible_pr_conversation",
                kind: "inline_and_commit",
                reason: (!selection.review()).then_some("Use --kind review or all; Web HTML access is required; cookie sign-in is only needed when the token/public read is denied."),
            },
            historical_review_complete: None,
            historical_review_limit: "GitBucket's PR conversation can omit reviews on excluded commits, groups overlapping storage IDs, and can show commit comments without exposing their stored PR association. This reader proves visibility, not complete stored history or PR membership.",
            web_actor,
            review_access,
            head_commit_id: head.map(|head|head.as_str().to_owned()),
        },
    })
}
