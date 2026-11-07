//! Conservative local PR checkout: never reset an existing user branch.
use crate::application::pull_request::load_saved_pull_request;
use crate::git::{snapshot, GitCommandExt};

use std::fs::{File, OpenOptions};
use std::path::PathBuf;
use std::process::{Command, Output};

use crate::cli::common::RepoContext;
use crate::error::{GbError, Result};

use super::git::{command_stderr, fetch_namespace, print_command_output, target_identity};

pub(super) async fn checkout(
    hostname: &Option<String>,
    cli_repo: &Option<String>,
    cli_profile: &Option<String>,
    number: u64,
) -> Result<()> {
    let ctx = RepoContext::resolve(hostname, cli_repo, cli_profile)?;
    let saved = load_saved_pull_request(&ctx.client, &ctx.owner, &ctx.repo, number).await?;

    let _lock = checkout_lock()?;
    ensure_clean_worktree()?;
    let branch = format!("pr-{number}");
    let identity = target_identity(&ctx, number);
    let key = format!("branch.{branch}.gb-pr-target");
    let previous_marker = config_value(&key)?;
    let existing = branch_tip(&branch)?;
    if existing.is_some() && previous_marker.as_deref() != Some(&identity) {
        return Err(GbError::Other(format!(
            "Refusing to reuse '{branch}': it is not recorded as belonging to this GitBucket repository and PR. Rename the existing branch yourself to keep it, then retry."
        )));
    }
    ensure_available(&branch)?;

    snapshot::ensure_commits(
        &ctx.git_auth,
        &saved.snapshot,
        saved.base_repository.as_ref(),
        saved.head_repository.as_ref(),
        &format!("{}/{}", ctx.owner, ctx.repo),
        &fetch_namespace(&identity),
        false,
    )?;
    let commit = saved.snapshot.head.as_str().to_string();
    // Recheck after network I/O. Git's non-forcing checkout also checks for races.
    ensure_clean_worktree()?;
    ensure_available(&branch)?;
    if branch_tip(&branch)? != existing {
        return Err(GbError::Other(format!(
            "Branch '{branch}' changed during fetch; retry after other Git operations finish."
        )));
    }
    if let Some(tip) = existing.as_deref() {
        if tip != commit {
            return Err(GbError::Other(format!(
                "Refusing to reset '{branch}': its commit differs from the saved PR head (local {tip}, PR {commit}). Keep or rename the branch and reconcile the changes manually; no automatic update is performed."
            )));
        }
    } else {
        // Record provenance before checkout so an unwritable config fails before
        // changing HEAD/files. Stale records alone never authorize resetting a tip.
        checked_git(&["config", "--local", &key, &identity])?;
    }
    let mut command = snapshot::command();
    command.args([
        "checkout",
        "--no-guess",
        "--no-overwrite-ignore",
        "--no-recurse-submodules",
    ]);
    if existing.is_none() {
        command.args(["--no-track", "-b", &branch, &commit]);
    } else {
        command.arg(&branch);
    }
    let result = command.arg("--").bounded_output();
    let completed = if matches!(&result, Ok(output) if output.status.success()) {
        true
    } else {
        match checkout_completed(&branch, &commit) {
            Ok(completed) => completed,
            // Cancellation/deadline failures may have changed HEAD and files.
            // A failed readback must not replace that mutation's error or
            // authorize removing its provenance record.
            Err(_) if result.is_err() => return result.map(|_| ()),
            Err(error) => {
                return Err(GbError::Unverified {
                    source: Box::new(error),
                })
            }
        }
    };
    if !completed && existing.is_none() {
        // Only metadata is restored; never reset/clean/stash user work to recover.
        let restored = match previous_marker {
            Some(value) => checked_git(&["config", "--local", &key, &value]),
            None => checked_git(&["config", "--local", "--unset-all", &key]),
        };
        if let Err(err) = restored {
            eprintln!("Warning: could not restore PR checkout metadata: {err}");
        }
    }
    let output = result?;
    if !output.status.success() {
        let context = if completed {
            "git checkout returned an error, but the requested branch and commit are active; inspect the working tree and hooks before retrying."
        } else {
            "git checkout failed."
        };
        return Err(GbError::Other(format!(
            "{context} {}",
            command_stderr(&output)
        )));
    }
    print_command_output(&output);
    println!(
        "✓ Checked out branch '{branch}' for PR #{number} at {commit} (saved head '{}')",
        saved
            .snapshot
            .head_branch
            .as_deref()
            .unwrap_or("saved commit")
    );
    Ok(())
}

fn checkout_completed(branch: &str, commit: &str) -> Result<bool> {
    let head = Command::new("git")
        .args(["symbolic-ref", "-q", "HEAD"])
        .bounded_output()?;
    if !head.status.success() {
        return Ok(false);
    }
    Ok(
        String::from_utf8_lossy(&head.stdout).trim_end() == format!("refs/heads/{branch}")
            && branch_tip(branch)?.as_deref() == Some(commit),
    )
}

fn checked_git(args: &[&str]) -> Result<Output> {
    let output = Command::new("git").args(args).bounded_output()?;
    if !output.status.success() {
        return Err(GbError::Other(format!(
            "git {} failed. {}",
            args[0],
            command_stderr(&output)
        )));
    }
    Ok(output)
}

fn config_value(key: &str) -> Result<Option<String>> {
    let output = Command::new("git")
        .args(["config", "--local", "--get", key])
        .bounded_output()?;
    match output.status.code() {
        Some(0) => Ok(Some(
            String::from_utf8_lossy(&output.stdout).trim_end().into(),
        )),
        Some(1) => Ok(None),
        _ => Err(GbError::Other(format!(
            "Cannot read PR checkout metadata. {}",
            command_stderr(&output)
        ))),
    }
}

fn branch_tip(branch: &str) -> Result<Option<String>> {
    let name = format!("refs/heads/{branch}");
    let output = checked_git(&["for-each-ref", "--format=%(refname) %(objectname)", &name])?;
    // Exact match only: a pr-7/topic ref is not a pr-7 branch.
    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .find_map(|line| {
            let (reference, sha) = line.split_once(' ')?;
            (reference == name).then(|| sha.to_string())
        }))
}

fn ensure_clean_worktree() -> Result<()> {
    let inside = checked_git(&["rev-parse", "--is-inside-work-tree"])?;
    if inside.stdout != b"true\n" {
        return Err(GbError::Other(
            "PR checkout requires a Git working tree.".into(),
        ));
    }
    let status = checked_git(&[
        "status",
        "--porcelain=v1",
        "-z",
        "--untracked-files=all",
        "--ignore-submodules=none",
    ])?;
    if !status.stdout.is_empty() {
        return Err(GbError::Other("PR checkout requires a clean working tree and index, including untracked files. Commit or move your changes before retrying; gb does not stash or discard them.".into()));
    }
    for state in [
        "MERGE_HEAD",
        "CHERRY_PICK_HEAD",
        "REVERT_HEAD",
        "rebase-merge",
        "rebase-apply",
        "sequencer",
        "BISECT_LOG",
    ] {
        let path = checked_git(&["rev-parse", "--git-path", state])?;
        if output_path(&path.stdout)?.exists() {
            return Err(GbError::Other("PR checkout refused while a merge, rebase, cherry-pick, revert or bisect is in progress. Finish or abort that operation first.".into()));
        }
    }
    Ok(())
}

fn ensure_available(branch: &str) -> Result<()> {
    let current = checked_git(&["rev-parse", "--show-toplevel"])?;
    let current = std::fs::canonicalize(output_path(&current.stdout)?)?;
    let name = format!("refs/heads/{branch}");
    // worktreepath and NUL atoms are supported by Git 2.34 as well as newer Git.
    let output = checked_git(&[
        "for-each-ref",
        "--format=%(refname)%00%(worktreepath)%00",
        &name,
    ])?;
    let mut fields = output.stdout.split(|byte| *byte == 0);
    while let (Some(reference), Some(path)) = (fields.next(), fields.next()) {
        let reference = reference.strip_prefix(b"\n").unwrap_or(reference);
        if reference == name.as_bytes()
            && !path.is_empty()
            && std::fs::canonicalize(raw_path(path)?)? != current
        {
            return Err(GbError::Other(format!("Branch '{branch}' is already checked out in another worktree. Switch branches there first.")));
        }
    }
    Ok(())
}

fn raw_path(bytes: &[u8]) -> Result<PathBuf> {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        Ok(PathBuf::from(std::ffi::OsStr::from_bytes(bytes)))
    }
    #[cfg(not(unix))]
    {
        Ok(PathBuf::from(std::str::from_utf8(bytes).map_err(|_| {
            GbError::Other("Git returned a non-UTF-8 path.".into())
        })?))
    }
}

fn output_path(bytes: &[u8]) -> Result<PathBuf> {
    let bytes = bytes.strip_suffix(b"\n").unwrap_or(bytes);
    #[cfg(windows)]
    let bytes = bytes.strip_suffix(b"\r").unwrap_or(bytes);
    raw_path(bytes)
}

fn checkout_lock() -> Result<File> {
    let common = checked_git(&["rev-parse", "--git-common-dir"])?;
    let path = output_path(&common.stdout)?.join("gb-pr-checkout.lock");
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)?;
    fs2::FileExt::try_lock_exclusive(&file).map_err(|err| {
        if err.raw_os_error() == fs2::lock_contended_error().raw_os_error() {
            GbError::Other("Another gb PR checkout is running in this repository. Retry after it finishes; do not delete gb-pr-checkout.lock.".into())
        } else { err.into() }
    })?;
    Ok(file)
}
