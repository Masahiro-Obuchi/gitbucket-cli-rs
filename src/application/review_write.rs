//! Verify writes against visible identity, exact raw body, coordinates and head.
use super::pull_request::{load_saved_pull_request, SavedPullRequest};
use crate::api::{client::ApiClient, web::GitBucketWebSession};
use crate::config::review_receipt::{self, Receipt};
use crate::domain::{pull_request::PullRequestSnapshot, review_comment::*, review_write::*};
use crate::error::{GbError, Result};
use crate::operation::{Progress, Route};
use serde::Serialize;

pub(crate) struct Writer<'a> {
    pub client: &'a ApiClient,
    pub session: &'a GitBucketWebSession,
    pub owner: &'a str,
    pub repo: &'a str,
    pub snapshot: &'a PullRequestSnapshot,
}

#[derive(Serialize)]
pub(crate) struct WriteResult {
    pub status: &'static str,
    pub action: &'static str,
    pub identity: CommentIdentity,
    pub web_actor: String,
    pub reused: bool,
    pub review: Option<ReviewComment>,
    pub reply_to: Option<CommentIdentity>,
    pub reply_semantics: Option<&'static str>,
    pub route: &'static str,
    pub verification: &'static str,
    pub base_commit_id: String,
    pub head_commit_id: String,
}

impl Writer<'_> {
    async fn check_snapshot(&self) -> Result<SavedPullRequest> {
        let actual =
            load_saved_pull_request(self.client, self.owner, self.repo, self.snapshot.number)
                .await?;
        if actual.snapshot.base != self.snapshot.base || actual.snapshot.head != self.snapshot.head
        {
            return Err(invalid("The saved PR comparison changed. No further write was attempted; inspect the PR before retrying."));
        }
        Ok(actual)
    }
    async fn check_open_snapshot(&self) -> Result<()> {
        let actual = self.check_snapshot().await?;
        if actual.state != "open" || actual.merged == Some(true) {
            return Err(invalid(
                "New inline comments/replies require an open, unmerged PR. No comment was posted.",
            ));
        }
        Ok(())
    }
    pub async fn rows(&self) -> Result<Vec<ReviewComment>> {
        self.session
            .review_comments(
                self.owner,
                self.repo,
                self.snapshot.number,
                &self.snapshot.head,
            )
            .await
    }
    pub fn selected<'a>(&self, rows: &'a [ReviewComment], id: u64) -> Result<&'a ReviewComment> {
        let row = rows
            .iter()
            .find(|row| row.identity.id == id && row.identity.kind == CommentKind::Inline)
            .ok_or_else(|| {
                invalid(concat!(
                    "The explicit inline ID is not visible on this PR. ",
                    "No IssueComment or hidden review ID will be substituted."
                ))
            })?;
        if row.visible_on_pull_request != self.snapshot.number
            || row.identity.repository != format!("{}/{}", self.owner, self.repo)
            || row.identity.instance != self.client.web_url("")
        {
            return Err(invalid(
                "Review identity does not match the selected instance/repository/PR.",
            ));
        }
        Ok(row)
    }
    pub fn current_position(&self, row: &ReviewComment) -> Result<InlinePosition> {
        if row.position.commit_id.as_deref() != Some(self.snapshot.head.as_str())
            || row.position.outdated != Some(false)
        {
            return Err(invalid("Outdated reviews cannot be used for inline writes; their original coordinates are not exposed by this server."));
        }
        let position = InlinePosition {
            path: row
                .position
                .path
                .clone()
                .ok_or_else(|| invalid("Inline review has no file path."))?,
            old_line: row.position.old_line,
            new_line: row.position.new_line,
        };
        validate_path(&position.path)?;
        Ok(position)
    }
    fn exact(&self, row: &ReviewComment, position: &InlinePosition) -> bool {
        row.identity.kind == CommentKind::Inline
            && row.author == self.session.principal()
            && row.position.commit_id.as_deref() == Some(self.snapshot.head.as_str())
            && row.position.outdated == Some(false)
            && row.position.path.as_deref() == Some(&position.path)
            && row.position.old_line == position.old_line
            && row.position.new_line == position.new_line
    }
    pub async fn raw_matches(&self, id: u64, body: &str) -> Result<bool> {
        let raw = self
            .session
            .review_edit_body(self.owner, self.repo, id)
            .await?;
        Ok(normalized_body(&raw)? == body)
    }
    fn result(
        &self,
        action: &'static str,
        row: ReviewComment,
        reused: bool,
        parent: Option<&ReviewComment>,
    ) -> WriteResult {
        WriteResult {
            status: "succeeded",
            action,
            identity: row.identity.clone(),
            web_actor: self.session.principal().into(),
            reused,
            review: Some(row),
            reply_to: parent.map(|r| r.identity.clone()),
            reply_semantics: parent.map(|_| "same_web_position_group_not_parent_id"),
            route: "web",
            verification: "saved_git_diff_and_raw_body",
            base_commit_id: self.snapshot.base.as_str().into(),
            head_commit_id: self.snapshot.head.as_str().into(),
        }
    }
    async fn confirmed(
        &self,
        receipt: &Receipt,
        id: Option<u64>,
        position: &InlinePosition,
        body: &str,
        parent: Option<&ReviewComment>,
    ) -> Result<ReviewComment> {
        let rows = self.rows().await?;
        let mut candidates = Vec::new();
        for row in rows {
            if !receipt.before_ids.contains(&row.identity.id)
                && id.is_none_or(|id| id == row.identity.id)
                && self.exact(&row, position)
                && self.raw_matches(row.identity.id, body).await?
            {
                candidates.push(row);
            }
        }
        if candidates.len() != 1 {
            return Err(invalid("Could not uniquely confirm the submitted review. The intent is reserved; it will not be posted again automatically. Inspect the PR before choosing a new --request-id."));
        }
        let row = candidates.remove(0);
        if let Some(parent) = parent {
            if row.thread.as_ref().map(|t| &t.id) != parent.thread.as_ref().map(|t| &t.id)
                || row.thread.is_none()
            {
                return Err(invalid(format!("Review {} was submitted, but the server did not confirm the selected position group. Do not repost; inspect the PR.",row.identity)));
            }
        }
        self.check_snapshot().await?;
        Ok(row)
    }
    pub async fn create(
        &self,
        before: &[ReviewComment],
        position: &InlinePosition,
        diff: &str,
        body: &str,
        parent: Option<&ReviewComment>,
        request_id: Option<&str>,
    ) -> Result<WriteResult> {
        if request_id.is_some_and(|id| id.is_empty() || id.len() > 128) {
            return Err(invalid("--request-id must contain 1 to 128 bytes."));
        }
        let scope = (
            &self.client.web_url(""),
            self.owner,
            self.repo,
            self.snapshot.number,
            self.session.principal(),
        );
        let fingerprint = review_receipt::digest(&(
            scope,
            self.snapshot.base.as_str(),
            self.snapshot.head.as_str(),
            position,
            body,
            parent.map(|r| r.identity.id),
        ))?;
        let intent = review_receipt::digest(&(position, body, parent.map(|r| r.identity.id)))?;
        let key =
            review_receipt::digest(&(scope, request_id.is_some(), request_id.unwrap_or(&intent)))?;
        self.check_snapshot().await?;
        let (receipt, fresh) = review_receipt::reserve(
            &key,
            fingerprint,
            before.iter().map(|r| r.identity.id).collect(),
        )?;
        let action = if parent.is_some() { "reply" } else { "create" };
        if !fresh {
            let row = self
                .confirmed(&receipt, None, position, body, parent)
                .await
                .map_err(|source| GbError::Unverified {
                    source: Box::new(source),
                })?;
            return Ok(self.result(action, row, true, parent));
        }
        // The server has no conditional write. A final race after this check
        // remains, so post-write comparison verification is also mandatory.
        self.check_open_snapshot().await?;
        let mut progress = Progress::default();
        let step = progress.plan(format!("review.{action}"), Route::Web);
        progress.actor(self.session.principal());
        let row = progress
            .run(
                step,
                async {
                    match self
                        .session
                        .create_inline(
                            self.owner,
                            self.repo,
                            InlineSubmission {
                                number: self.snapshot.number,
                                head: &self.snapshot.head,
                                position,
                                body,
                                diff,
                            },
                        )
                        .await
                    {
                        Ok(id) => Ok((Some(id), None)),
                        Err(
                            error @ (GbError::Api {
                                status: 400..=499, ..
                            }
                            | GbError::Auth(_)),
                        ) => Err(error),
                        // Reconcile once using only reads. Never repeat the POST.
                        Err(error) => Ok((None, Some(error))),
                    }
                },
                |(id, submission_error)| async move {
                    self.confirmed(&receipt, id, position, body, parent)
                        .await
                        .map_err(|verification_error| {
                            retain_submission_error(submission_error, verification_error)
                        })
                },
            )
            .await?;
        Ok(self.result(action, row, false, parent))
    }
    pub async fn change(
        &self,
        target: &ReviewComment,
        position: &InlinePosition,
        body: Option<&str>,
    ) -> Result<WriteResult> {
        if target.author != self.session.principal() {
            return Err(invalid(
                "Editing/deleting is limited to the verified Web actor's own inline comments.",
            ));
        }
        // The permission-gated form establishes editability and supplies raw text.
        let old = self
            .session
            .review_edit_body(self.owner, self.repo, target.identity.id)
            .await?;
        if body.is_some_and(|body| normalized_body(&old).is_ok_and(|old| old == body)) {
            self.check_snapshot().await?;
            return Ok(self.result("edit", target.clone(), true, None));
        }
        self.check_snapshot().await?;
        let id = target.identity.id;
        let action = if body.is_some() { "edit" } else { "delete" };
        let mut progress = Progress::default();
        let step = progress.plan(format!("review.{action}:{id}"), Route::Web);
        progress.actor(self.session.principal());
        let row = progress
            .run(
                step,
                self.submit_change(id, body),
                |submission_error| async {
                    self.confirm_change(id, position, body)
                        .await
                        .map_err(|verification_error| {
                            retain_submission_error(submission_error, verification_error)
                        })
                },
            )
            .await?;
        Ok(WriteResult {
            status: "succeeded",
            action,
            identity: target.identity.clone(),
            web_actor: self.session.principal().into(),
            reused: false,
            review: row,
            reply_to: None,
            reply_semantics: None,
            route: "web",
            verification: if body.is_some() {
                "saved_git_diff_and_raw_body"
            } else {
                "saved_git_diff_and_resource_absence"
            },
            base_commit_id: self.snapshot.base.as_str().into(),
            head_commit_id: self.snapshot.head.as_str().into(),
        })
    }

    async fn submit_change(&self, id: u64, body: Option<&str>) -> Result<Option<GbError>> {
        let result = match body {
            Some(body) => {
                self.session
                    .edit_inline(self.owner, self.repo, id, body)
                    .await
            }
            None => self.session.delete_inline(self.owner, self.repo, id).await,
        };
        match result {
            Ok(()) => Ok(None),
            Err(
                error @ (GbError::Api {
                    status: 400..=499, ..
                }
                | GbError::Auth(_)),
            ) => Err(error),
            Err(error) => Ok(Some(error)),
        }
    }

    async fn confirm_change(
        &self,
        id: u64,
        position: &InlinePosition,
        body: Option<&str>,
    ) -> Result<Option<ReviewComment>> {
        let rows = self.rows().await?;
        let result = if let Some(body) = body {
            let row = self.selected(&rows, id)?;
            if !self.exact(row, position) || !self.raw_matches(id, body).await? {
                return Err(invalid("Stored inline body/identity/position did not confirm the edit. No automatic retry was attempted."));
            }
            Some(row.clone())
        } else {
            if rows.iter().any(|r| r.identity.id == id)
                || !self
                    .session
                    .review_is_deleted(self.owner, self.repo, id)
                    .await?
            {
                return Err(invalid(
                    "Deletion was not confirmed by both conversation absence and a resource 404.",
                ));
            }
            None
        };
        self.check_snapshot().await?;
        Ok(result)
    }
}

fn retain_submission_error(submission: Option<GbError>, verification: GbError) -> GbError {
    match submission {
        Some(source) => {
            let context = format!(
                "Review outcome remains unverified; readback also failed: {verification}. \
                 No automatic POST retry was attempted"
            );
            GbError::Context {
                source: Box::new(source),
                context,
            }
        }
        None => verification,
    }
}
