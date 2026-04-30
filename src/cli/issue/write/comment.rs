use dialoguer::Input;

use crate::cli::common::RepoContext;
use crate::cli::issue_like::latest_comment_by_current_user;
use crate::error::Result;
use crate::models::comment::CreateComment;

pub(in crate::cli::issue) async fn comment(
    hostname: &Option<String>,
    cli_repo: &Option<String>,
    cli_profile: &Option<String>,
    number: u64,
    body: Option<String>,
    edit_last: bool,
) -> Result<()> {
    let ctx = RepoContext::resolve(hostname, cli_repo, cli_profile)?;

    let body_text = match body {
        Some(body) => body,
        None => Input::new().with_prompt("Comment body").interact_text()?,
    };

    let comment_body = CreateComment { body: body_text };
    if edit_last {
        let comment =
            latest_comment_by_current_user(&ctx.client, &ctx.owner, &ctx.repo, number, "issue")
                .await?;

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
