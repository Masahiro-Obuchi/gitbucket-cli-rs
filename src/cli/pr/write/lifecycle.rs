use crate::cli::common::RepoContext;
use crate::error::Result;

pub(in crate::cli::pr) async fn close(
    hostname: &Option<String>,
    cli_repo: &Option<String>,
    cli_profile: &Option<String>,
    number: u64,
) -> Result<()> {
    super::edit::apply_edit(
        hostname,
        cli_repo,
        cli_profile,
        super::edit::EditRequest::state(number, "closed"),
    )
    .await?;
    println!("✓ Closed pull request #{}", number);
    Ok(())
}

pub(in crate::cli::pr) async fn reopen(
    hostname: &Option<String>,
    cli_repo: &Option<String>,
    cli_profile: &Option<String>,
    number: u64,
) -> Result<()> {
    super::edit::apply_edit(
        hostname,
        cli_repo,
        cli_profile,
        super::edit::EditRequest::state(number, "open"),
    )
    .await?;
    println!("✓ Reopened pull request #{}", number);
    Ok(())
}

pub(in crate::cli::pr) async fn merge(
    hostname: &Option<String>,
    cli_repo: &Option<String>,
    cli_profile: &Option<String>,
    number: u64,
    message: Option<String>,
    expected_head: Option<crate::domain::pull_request::CommitId>,
    json: bool,
) -> Result<()> {
    let ctx = RepoContext::resolve(hostname, cli_repo, cli_profile)?;
    eprintln!("Notice: Merge uses client head checks only. GitBucket 4.44.0 has no atomic expected-head guard; a later branch update can still race the merge.");
    let report = crate::application::pr_merge::merge(
        &ctx.client,
        &ctx.owner,
        &ctx.repo,
        number,
        expected_head,
        message,
    )
    .await?;
    if json {
        return crate::output::print_json(&report);
    }
    println!("✓ Merged pull request #{}", number);
    println!(
        "Checked head: {} (client checks; not atomic)",
        report.head_commit
    );
    Ok(())
}
