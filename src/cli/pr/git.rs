use crate::git::GitCommandExt;
use std::process::{Command, Output};

use crate::cli::common::RepoContext;
use crate::error::Result;
use crate::output;

pub(super) fn current_branch_name() -> Result<Option<String>> {
    let output = Command::new("git")
        .args(["branch", "--show-current"])
        .bounded_output()?;
    let branch = String::from_utf8_lossy(&output.stdout).trim().to_string();
    Ok((output.status.success() && !branch.is_empty()).then_some(branch))
}

pub(super) fn target_identity(ctx: &RepoContext, number: u64) -> String {
    serde_json::to_string(&(ctx.client.web_url(""), &ctx.owner, &ctx.repo, number)).unwrap()
}

pub(super) fn fetch_namespace(identity: &str) -> String {
    let digest = ring::digest::digest(&ring::digest::SHA256, identity.as_bytes());
    let encoded: String = digest
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    format!("refs/gb/pr-targets/v1/{encoded}")
}

pub(super) fn print_command_output(output: &Output) {
    if !output.stdout.is_empty() {
        print!("{}", String::from_utf8_lossy(&output.stdout));
    }
    if !output.stderr.is_empty() && !output::suppress_stderr() {
        eprint!("{}", String::from_utf8_lossy(&output.stderr));
    }
}

pub(super) fn command_stderr(output: &Output) -> String {
    let stderr = String::from_utf8_lossy(&output.stderr);
    let trimmed = stderr.trim();
    if trimmed.is_empty() {
        return "git did not provide stderr details.".into();
    }
    trimmed.lines().take(3).collect::<Vec<_>>().join(" ")
}
