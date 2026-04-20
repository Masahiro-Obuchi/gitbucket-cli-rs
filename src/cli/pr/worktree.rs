use crate::cli::common::{create_client, resolve_hostname, resolve_repo};
use crate::error::{GbError, Result};

use super::git::{pr_base_fetch_source, pr_head_fetch_source, resolve_git_fetch_source};

pub(super) async fn checkout(
    hostname: &Option<String>,
    cli_repo: &Option<String>,
    number: u64,
) -> Result<()> {
    let hostname = resolve_hostname(hostname)?;
    let (owner, repo) = resolve_repo(cli_repo)?;
    let client = create_client(&hostname)?;

    let pr = client.get_pull_request(&owner, &repo, number).await?;
    let branch = pr
        .head
        .as_ref()
        .map(|h| h.ref_name.as_str())
        .ok_or_else(|| GbError::Other("PR has no head branch".into()))?;
    let local_branch = format!("pr-{}", number);

    let fetch_source = resolve_git_fetch_source(&hostname, &pr_head_fetch_source(&pr));
    let head_ref = format!("refs/gb/pr/{}/head", number);

    let fetch_output = std::process::Command::new("git")
        .env("GIT_TERMINAL_PROMPT", "0")
        .arg("fetch")
        .arg(&fetch_source.command_source)
        .arg(format!("{}:{}", branch, head_ref))
        .output()?;

    if !fetch_output.status.success() {
        return Err(GbError::Other(format!(
            "git fetch failed for '{}' from {}",
            branch, fetch_source.display_source
        )));
    }

    let checkout_status = std::process::Command::new("git")
        .arg("checkout")
        .arg("-B")
        .arg(&local_branch)
        .arg(&head_ref)
        .status()?;

    if !checkout_status.success() {
        return Err(GbError::Other("git checkout failed".into()));
    }

    println!(
        "✓ Checked out branch '{}' for PR #{} (from '{}')",
        local_branch, number, branch
    );
    Ok(())
}

pub(super) async fn diff(
    hostname: &Option<String>,
    cli_repo: &Option<String>,
    number: u64,
    no_pager: bool,
) -> Result<()> {
    let hostname = resolve_hostname(hostname)?;
    let (owner, repo) = resolve_repo(cli_repo)?;
    let client = create_client(&hostname)?;

    let pr = client.get_pull_request(&owner, &repo, number).await?;
    let head = pr
        .head
        .as_ref()
        .map(|h| h.ref_name.as_str())
        .unwrap_or("HEAD");
    let base = pr
        .base
        .as_ref()
        .map(|b| b.ref_name.as_str())
        .unwrap_or("main");

    let fetch_source = resolve_git_fetch_source(&hostname, &pr_head_fetch_source(&pr));
    let base_fetch_source = resolve_git_fetch_source(&hostname, &pr_base_fetch_source(&pr));
    let base_ref = format!("refs/gb/pr/{}/base", number);
    let head_ref = format!("refs/gb/pr/{}/head", number);

    let base_fetch = std::process::Command::new("git")
        .env("GIT_TERMINAL_PROMPT", "0")
        .arg("fetch")
        .arg(&base_fetch_source.command_source)
        .arg(format!("{}:{}", base, base_ref))
        .output()?;
    if !base_fetch.status.success() {
        return Err(GbError::Other(format!(
            "git fetch failed for '{}' from {}",
            base, base_fetch_source.display_source
        )));
    }

    let head_fetch = std::process::Command::new("git")
        .env("GIT_TERMINAL_PROMPT", "0")
        .arg("fetch")
        .arg(&fetch_source.command_source)
        .arg(format!("{}:{}", head, head_ref))
        .output()?;
    if !head_fetch.status.success() {
        return Err(GbError::Other(format!(
            "git fetch failed for '{}' from {}",
            head, fetch_source.display_source
        )));
    }

    let mut diff_command = std::process::Command::new("git");
    if no_pager {
        diff_command.env("GIT_PAGER", "cat");
        diff_command.arg("--no-pager");
    }
    let status = diff_command
        .arg("diff")
        .arg(format!("{}...{}", base_ref, head_ref))
        .output()?;

    if !status.status.success() {
        return Err(GbError::Other("git diff failed".into()));
    }

    print!("{}", String::from_utf8_lossy(&status.stdout));
    eprint!("{}", String::from_utf8_lossy(&status.stderr));

    Ok(())
}
