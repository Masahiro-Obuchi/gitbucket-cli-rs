use crate::cli::common::RepoContext;
use crate::cli::issue_like::latest_comment_by_current_user;
use crate::error::Result;
use crate::models::comment::CreateComment;

pub(in crate::cli::issue) async fn comment(
    hostname: &Option<String>,
    cli_repo: &Option<String>,
    cli_profile: &Option<String>,
    number: u64,
    body: crate::cli::input::BodyInput,
    edit_last: bool,
) -> Result<()> {
    let ctx = RepoContext::resolve(hostname, cli_repo, cli_profile)?;

    let existing = if edit_last && body.is_editor() {
        Some(
            latest_comment_by_current_user(&ctx.client, &ctx.owner, &ctx.repo, number, "issue")
                .await?,
        )
    } else {
        None
    };
    let seed = existing
        .as_ref()
        .and_then(|comment| comment.body.as_deref())
        .unwrap_or_default();
    let body_text = crate::cli::input::comment_seed(body, seed).await?;

    let comment_body = CreateComment { body: body_text };
    if edit_last {
        let comment = match existing {
            Some(comment) => comment,
            None => {
                latest_comment_by_current_user(&ctx.client, &ctx.owner, &ctx.repo, number, "issue")
                    .await?
            }
        };

        ctx.client
            .update_issue_comment(&ctx.owner, &ctx.repo, comment.id, &comment_body)
            .await?;
        println!("✓ Edited comment {} on issue #{}", comment.id, number);
    } else {
        ctx.client
            .create_issue_comment(&ctx.owner, &ctx.repo, number, &comment_body)
            .await?;
        println!("✓ Added comment to issue #{}", number);
    }
    Ok(())
}
