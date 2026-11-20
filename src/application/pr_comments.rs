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
) -> Result<(PullRequest, Option<CommitId>)> {
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
        .and_then(|sha| CommitId::parse(sha).ok());
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
    pub comments: Option<Vec<Comment>>,
    pub review_comments: Option<Vec<ReviewComment>>,
    pub comment_coverage: CommentCoverage,
}

pub(crate) struct ReviewRead {
    pub comments: Vec<ReviewComment>,
    pub web_actor: Option<String>,
    pub head_commit_id: String,
}

pub(crate) fn combine(
    client: &ApiClient,
    owner: &str,
    repo: &str,
    selection: CommentSelection,
    discussion: Option<Result<Vec<Comment>>>,
    review: Option<Result<ReviewRead>>,
) -> Result<PrComments> {
    let review = match review {
        Some(Err(error)) if error.code() == "cancelled" => return Err(error),
        result => result,
    };
    let partial_allowed = discussion.as_ref().is_some_and(Result::is_ok)
        || review.as_ref().is_some_and(Result::is_ok);
    let (comments, discussion_status, discussion_reason) =
        source_result(discussion, partial_allowed, "complete")?;
    let (review, review_status, review_reason) =
        source_result(review, partial_allowed, "complete_for_scope")?;
    let review_access = match &review {
        Some(review) if review.web_actor.is_some() => "cookie_session",
        Some(_) => "token_or_public",
        None => review_status,
    };
    let (review_comments, web_actor, head_commit_id) = review
        .map(|r| (Some(r.comments), r.web_actor, Some(r.head_commit_id)))
        .unwrap_or_default();
    Ok(PrComments {
        comments,
        review_comments,
        comment_coverage: CommentCoverage {
            instance: client.web_url(""),
            requested: selection,
            repository: format!("{owner}/{repo}"),
            discussion: SourceCoverage {
                source_contract: "issue_discussion_rest",
                status: discussion_status,
                scope: "issue_discussion_api",
                kind: "discussion",
                reason: discussion_reason,
            },
            review: SourceCoverage {
                source_contract: "gitbucket_4_44_conversation_html",
                status: review_status,
                scope: "visible_pr_conversation",
                kind: "inline_and_commit",
                reason: review_reason,
            },
            historical_review_complete: None,
            historical_review_limit: "GitBucket's PR conversation can omit reviews on excluded commits, groups overlapping storage IDs, and can show commit comments without exposing their stored PR association. This reader proves visibility, not complete stored history or PR membership.",
            web_actor,
            review_access,
            head_commit_id,
        },
    })
}

fn source_result<T>(
    result: Option<Result<T>>,
    partial_allowed: bool,
    complete: &'static str,
) -> Result<(Option<T>, &'static str, Option<&'static str>)> {
    match result {
        None => Ok((None, "not_requested", None)),
        Some(Ok(value)) => Ok((Some(value), complete, None)),
        Some(Err(error)) if partial_allowed && error.code() != "cancelled" => Ok((
            None,
            "unavailable",
            Some(match error.status() {
                Some(401 | 403) => "access_denied",
                _ => error.cause_code().unwrap_or_else(|| error.code()),
            }),
        )),
        Some(Err(error)) => Err(error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn review_cancellation_takes_precedence_over_a_failed_discussion_read() {
        let client = ApiClient::new("https://bucket.example", "fixture-token", "https").unwrap();
        let error = combine(
            &client,
            "alice",
            "project",
            CommentSelection::All,
            Some(Err(GbError::Api {
                status: 500,
                message: "Discussion unavailable".into(),
            })),
            Some(Err(GbError::Cancelled)),
        )
        .err()
        .expect("cancelled collection");
        assert_eq!(error.code(), "cancelled");
    }
}
