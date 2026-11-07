use std::io::{Seek, SeekFrom};

use crate::application::pull_request::load_saved_pull_request;
use crate::cli::common::RepoContext;
use crate::error::{GbError, Result};
use crate::git::{snapshot, GitCommandExt};

use super::git::{fetch_namespace, target_identity};

pub(super) async fn diff(
    hostname: &Option<String>,
    cli_repo: &Option<String>,
    cli_profile: &Option<String>,
    number: u64,
    no_pager: bool,
) -> Result<()> {
    let ctx = RepoContext::resolve(hostname, cli_repo, cli_profile)?;
    let saved = load_saved_pull_request(&ctx.client, &ctx.owner, &ctx.repo, number).await?;
    let namespace = fetch_namespace(&target_identity(&ctx, number));
    snapshot::ensure_commits(
        &ctx.git_auth,
        &saved.snapshot,
        saved.base_repository.as_ref(),
        saved.head_repository.as_ref(),
        &format!("{}/{}", ctx.owner, ctx.repo),
        &namespace,
        true,
    )?;
    // 4.44's base.sha is the saved comparison starting commit. Compare exact trees;
    // never recalculate against moving tips or substitute an unverified .diff.
    let mut patch = tempfile::tempfile()?;
    let output = snapshot::command()
        .args([
            "--no-pager",
            "diff",
            "--binary",
            "--full-index",
            "--no-ext-diff",
            "--no-textconv",
            "--no-color",
            "--find-renames",
            "--no-relative",
            "--ignore-submodules=none",
            "--submodule=short",
            saved.snapshot.base.as_str(),
            saved.snapshot.head.as_str(),
            "--",
        ])
        .bounded_output_to_file(patch.try_clone()?)?;
    if !output.status.success() {
        return Err(GbError::DiffUnavailable {
            number, cause: "git_diff_failed", message: "Git could not compare the verified saved commit trees. No partial diff was printed.".into(),
        });
    }
    patch.seek(SeekFrom::Start(0))?;
    crate::output::page_reader(&mut patch, no_pager)?;
    Ok(())
}
