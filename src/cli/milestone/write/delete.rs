use dialoguer::Confirm;

use crate::cli::common::{create_web_session, RepoContext};
use crate::error::{GbError, Result};

pub(in crate::cli::milestone) async fn delete(
    hostname: &Option<String>,
    cli_repo: &Option<String>,
    cli_profile: &Option<String>,
    number: u64,
    yes: bool,
) -> Result<()> {
    let ctx = RepoContext::resolve(hostname, cli_repo, cli_profile)?;

    if !yes {
        let confirmed = Confirm::new()
            .with_prompt(format!(
                "Are you sure you want to delete milestone #{} from {}/{}?",
                number, ctx.owner, ctx.repo
            ))
            .default(false)
            .interact()?;
        if !confirmed {
            println!("Aborted.");
            return Ok(());
        }
    }

    match ctx
        .client
        .delete_milestone(&ctx.owner, &ctx.repo, number)
        .await
    {
        Ok(()) => {
            println!("✓ Deleted milestone #{}", number);
            Ok(())
        }
        Err(GbError::Api { status: 404, .. }) => {
            eprintln!(
                "Notice: REST milestone delete is unavailable on this GitBucket instance; using web fallback."
            );
            let session = create_web_session(&ctx.hostname, cli_profile).await?;
            session
                .delete_milestone(&ctx.owner, &ctx.repo, number)
                .await?;
            println!("✓ Deleted milestone #{}", number);
            Ok(())
        }
        Err(err) => Err(err),
    }
}
