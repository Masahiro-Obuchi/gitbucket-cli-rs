use dialoguer::Confirm;

use crate::cli::common::RepoContext;
use crate::error::Result;

pub(in crate::cli::milestone) async fn delete(
    hostname: &Option<String>,
    cli_repo: &Option<String>,
    cli_profile: &Option<String>,
    number: u64,
    yes: bool,
) -> Result<()> {
    let ctx = RepoContext::resolve(hostname, cli_repo, cli_profile)?;

    if !yes {
        let prompt = format!(
            "Are you sure you want to delete milestone #{} from {}/{}?",
            number, ctx.owner, ctx.repo
        );
        let confirmed = crate::cancel::input(move || {
            Confirm::new().with_prompt(prompt).default(false).interact()
        })
        .await?;
        if !confirmed {
            println!("Aborted.");
            return Ok(());
        }
    }

    ctx.client
        .delete_milestone(&ctx.owner, &ctx.repo, number)
        .await?;
    println!("✓ Deleted milestone #{}", number);
    Ok(())
}
