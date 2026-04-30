use dialoguer::Input;

use crate::cli::common::RepoContext;
use crate::cli::issue_like::latest_comment_by_current_user;
use crate::error::Result;
use crate::models::comment::{Comment, CreateComment};
use crate::output;

pub(in crate::cli::pr) async fn comment(
    hostname: &Option<String>,
    cli_repo: &Option<String>,
    cli_profile: &Option<String>,
    number: u64,
    body: Option<String>,
    edit_last: bool,
    json: bool,
) -> Result<()> {
    let ctx = RepoContext::resolve(hostname, cli_repo, cli_profile)?;

    let body_text = match body {
        Some(b) => b,
        None => Input::new().with_prompt("Comment body").interact_text()?,
    };

    let comment_body = CreateComment { body: body_text };
    if edit_last {
        let comment =
            latest_comment_by_current_user(&ctx.client, &ctx.owner, &ctx.repo, number, "PR")
                .await?;

        let comment = ctx
            .client
            .update_issue_comment(&ctx.owner, &ctx.repo, comment.id, &comment_body)
            .await?;
        print_comment_result(&comment, number, true, json)?;
    } else {
        let comment = ctx
            .client
            .create_pr_comment(&ctx.owner, &ctx.repo, number, &comment_body)
            .await?;
        print_comment_result(&comment, number, false, json)?;
    }
    Ok(())
}

fn print_comment_result(comment: &Comment, pr_number: u64, edited: bool, json: bool) -> Result<()> {
    if json {
        return output::print_json(comment);
    }

    let action = if edited { "Edited" } else { "Added" };
    println!("✓ {} comment {} on PR #{}", action, comment.id, pr_number);
    if let Some(url) = comment.html_url.as_deref().filter(|url| !url.is_empty()) {
        println!("URL: {}", url);
    }
    Ok(())
}
