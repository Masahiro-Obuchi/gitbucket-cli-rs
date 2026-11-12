//! CLI input/authentication and Git adapter composition for review mutations.
use super::{PrCommentArgs, ReviewTargetArgs};
use crate::application::{pull_request::load_saved_pull_request, review_write::Writer};
use crate::cli::common::{create_web_session, RepoContext};
use crate::domain::{pull_request::CommitId, review_write::*};
use crate::error::Result;
use crate::git::{review_position, snapshot};
use std::path::PathBuf;

#[derive(Clone, Copy, PartialEq)]
pub(super) enum Action {
    Create,
    Edit,
    Reply,
    Delete,
}
pub(super) struct Request {
    number: u64,
    action: Action,
    id: Option<u64>,
    position: Option<(String, u64, Side)>,
    body: Option<String>,
    body_file: Option<PathBuf>,
    commit: Option<String>,
    request_id: Option<String>,
    json: bool,
}
impl Request {
    pub fn identified(
        target: ReviewTargetArgs,
        action: Action,
        body: Option<String>,
        body_file: Option<PathBuf>,
        request_id: Option<String>,
    ) -> Self {
        Self {
            number: target.number,
            action,
            id: Some(target.id),
            position: None,
            body,
            body_file,
            commit: target.commit,
            request_id,
            json: target.json,
        }
    }
    pub fn create(number: u64, args: PrCommentArgs) -> Result<Self> {
        let position = match (args.file, args.line, args.side, args.web) {
            (Some(path), Some(line), Some(side), true) => (path, line, side),
            _ => {
                return Err(invalid(
                    "Inline writes require --file, --line, --side and --web.",
                ))
            }
        };
        Ok(Self {
            number,
            action: Action::Create,
            id: None,
            position: Some(position),
            body: args.body,
            body_file: args.body_file,
            commit: args.commit,
            request_id: args.request_id,
            json: args.json,
        })
    }
}
pub(super) async fn run(
    host: &Option<String>,
    repo: &Option<String>,
    profile: &Option<String>,
    request: Request,
) -> Result<()> {
    let body = if request.action == Action::Delete {
        None
    } else {
        let text = crate::cli::input::comment(
            crate::cli::input::body(request.body, request.body_file).await?,
        )
        .await?;
        Some(normalized_body(&text)?)
    };
    let ctx = RepoContext::resolve(host, repo, profile)?;
    let pr = super::target::require_pull_request(&ctx, request.number).await?;
    if matches!(request.action, Action::Create | Action::Reply)
        && (pr.state != "open" || pr.merged == Some(true))
    {
        return Err(invalid(
            "New inline comments/replies require an open, unmerged PR.",
        ));
    }
    let saved = load_saved_pull_request(&ctx.client, &ctx.owner, &ctx.repo, request.number).await?;
    if let Some(expected) = request.commit {
        if CommitId::parse(&expected)? != saved.snapshot.head {
            return Err(invalid(
                "--commit does not match the saved PR head. No Web write was attempted.",
            ));
        }
    }
    let namespace = super::git::fetch_namespace(&super::git::target_identity(&ctx, request.number));
    snapshot::ensure_commits(
        &ctx.git_auth,
        &saved.snapshot,
        saved.base_repository.as_ref(),
        saved.head_repository.as_ref(),
        &format!("{}/{}", ctx.owner, ctx.repo),
        &namespace,
        true,
    )?;
    // Validate explicit file coordinates before asking for Web credentials.
    let line = if let Some((path, line, side)) = &request.position {
        Some(review_position::verify(
            &saved.snapshot,
            path,
            *line,
            *side,
        )?)
    } else {
        None
    };
    eprintln!("Notice: Inline writes use the verified Cookie Web actor and saved PR diff. CommitComment records can also be visible outside this PR. Replies add at a position group; no parent ID is stored.");
    let session = create_web_session(&ctx.client, &ctx.hostname, profile).await?;
    let writer = Writer {
        client: &ctx.client,
        session: &session,
        owner: &ctx.owner,
        repo: &ctx.repo,
        snapshot: &saved.snapshot,
    };
    let before = writer.rows().await?;
    let target = request
        .id
        .map(|id| writer.selected(&before, id))
        .transpose()?;
    let mut position = line.as_ref().map(|line| line.position.clone());
    let mut diff = line.map(|line| line.diff);
    if let Some(target) = target {
        if request.action != Action::Reply && target.author != session.principal() {
            return Err(invalid(
                "Editing/deleting is limited to your own inline comments.",
            ));
        }
        let actual = writer.current_position(target)?;
        let (line, side) = match (actual.new_line, actual.old_line) {
            (Some(line), _) => (line, Side::New),
            (None, Some(line)) => (line, Side::Old),
            _ => {
                return Err(invalid(
                    "File-level reviews have no line to validate for writing.",
                ))
            }
        };
        let verified = review_position::verify(&saved.snapshot, &actual.path, line, side)?;
        if actual
            .old_line
            .is_some_and(|line| Some(line) != verified.position.old_line)
            || actual
                .new_line
                .is_some_and(|line| Some(line) != verified.position.new_line)
        {
            return Err(invalid(
                "Displayed review coordinates do not match the saved Git diff.",
            ));
        }
        position = Some(actual);
        diff = Some(verified.diff);
    }
    let position = position.ok_or_else(|| invalid("Missing verified inline position."))?;
    let result = match request.action {
        Action::Create | Action::Reply => {
            writer
                .create(
                    &before,
                    &position,
                    diff.as_deref()
                        .ok_or_else(|| invalid("Missing verified diff snippet."))?,
                    body.as_deref().unwrap_or_default(),
                    target,
                    request.request_id.as_deref(),
                )
                .await?
        }
        Action::Edit | Action::Delete => {
            writer
                .change(
                    target.ok_or_else(|| invalid("Missing explicit inline identity."))?,
                    &position,
                    body.as_deref(),
                )
                .await?
        }
    };
    if request.json {
        crate::output::print_json(&result)?;
    } else {
        println!(
            "✓ {} {} on PR #{}{}",
            result.action,
            result.identity,
            request.number,
            if result.reused {
                " (existing confirmed result)"
            } else {
                ""
            }
        );
        if let Some(row) = &result.review {
            println!("URL: {}", row.html_url);
        }
    }
    Ok(())
}
