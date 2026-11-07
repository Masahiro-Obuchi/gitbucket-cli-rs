//! Fetch only to private refs, then use full verified object IDs, never tips.
use std::process::{Command, Output};

use super::{GitAuth, GitCommandExt};
use crate::domain::pull_request::{CommitId, PullRequestSnapshot};
use crate::error::{GbError, Result};
use crate::models::repository::Repository;

pub(crate) fn command() -> Command {
    let mut command = Command::new("git");
    command.env("GIT_NO_REPLACE_OBJECTS", "1");
    command
}

fn has_commit(id: &CommitId) -> Result<bool> {
    let output = command()
        .args([
            "rev-parse",
            "--verify",
            "--end-of-options",
            &format!("{}^{{commit}}", id.as_str()),
        ])
        .bounded_output()?;
    Ok(output.status.success() && String::from_utf8_lossy(&output.stdout).trim() == id.as_str())
}

fn source(auth: &GitAuth, repo: Option<&Repository>, fallback: &str) -> Result<String> {
    let url = repo
        .map(|repo| auth.clone_url(repo))
        .unwrap_or_else(|| auth.repository_url(fallback, None));
    let remotes = command().arg("remote").bounded_output()?;
    if remotes.status.success() {
        for name in String::from_utf8_lossy(&remotes.stdout).lines() {
            let remote = command()
                .args(["config", "--get", &format!("remote.{name}.url")])
                .bounded_output()?;
            let remote = String::from_utf8_lossy(&remote.stdout).trim().to_string();
            if super::repository_urls_match(&url, &remote) {
                return super::clean_source(&remote);
            }
        }
    }
    super::clean_source(&url)
}

fn fetch(auth: &GitAuth, url: &str, want: &str, target: &str) -> Result<Output> {
    let mut output = auth
        .command(url)?
        .env("GIT_NO_REPLACE_OBJECTS", "1")
        .arg("fetch")
        .args([
            "--no-tags",
            "--no-write-fetch-head",
            "--no-recurse-submodules",
            "--refmap=",
            "--",
        ])
        .arg(url)
        .arg(format!("+{want}:{target}"))
        .bounded_output()?;
    auth.redact_output(&mut output);
    Ok(output)
}

fn branch_ref(branch: Option<&str>) -> Result<Option<String>> {
    let Some(branch) = branch else {
        return Ok(None);
    };
    let reference = format!("refs/heads/{branch}");
    let valid = command()
        .args(["check-ref-format", &reference])
        .bounded_output()?;
    Ok(valid.status.success().then_some(reference))
}

pub(crate) fn ensure_commits(
    auth: &GitAuth,
    snapshot: &PullRequestSnapshot,
    base_repo: Option<&Repository>,
    head_repo: Option<&Repository>,
    selected_repo: &str,
    namespace: &str,
    need_base: bool,
) -> Result<()> {
    for branch in [&snapshot.head_branch, &snapshot.base_branch] {
        if branch.is_some() && branch_ref(branch.as_deref())?.is_none() {
            return Err(GbError::Other(
                "API returned an invalid PR branch ref.".into(),
            ));
        }
    }
    if !command()
        .args(["rev-parse", "--git-dir"])
        .bounded_output()?
        .status
        .success()
    {
        return Err(GbError::DiffUnavailable {
            number: snapshot.number,
            cause: "git_repository_unavailable",
            message: "Run this operation inside a local Git repository so saved PR objects can be verified.".into(),
        });
    }
    let mut base_source = None;
    for (role, id, branch) in [
        ("head", &snapshot.head, snapshot.head_branch.as_deref()),
        ("base", &snapshot.base, snapshot.base_branch.as_deref()),
    ] {
        if role == "base" && !need_base {
            break;
        }
        if !has_commit(id)? {
            let base_url = match &base_source {
                Some(url) => url,
                None => base_source.insert(source(auth, base_repo, selected_repo)?),
            };
            let mut candidates = Vec::new();
            if role == "head" {
                candidates.push((
                    base_url.clone(),
                    format!("refs/pull/{}/head", snapshot.number),
                ));
            }
            candidates.push((base_url.clone(), id.as_str().to_string()));
            if role == "base" {
                if let Some(reference) = branch_ref(branch)? {
                    candidates.push((base_url.clone(), reference));
                }
            } else if let Some(repo) = head_repo {
                let url = source(auth, Some(repo), &repo.full_name)?;
                candidates.push((url.clone(), id.as_str().to_string()));
                if let Some(reference) = branch_ref(branch)? {
                    candidates.push((url, reference));
                }
            } else if let Some(reference) = branch_ref(branch)? {
                // The selected repository may itself own the head. We only use
                // an expected object after fetching; a same-name tip is not proof.
                candidates.push((base_url.clone(), reference));
            }
            for (url, want) in candidates {
                let _ = fetch(auth, &url, &want, &format!("{namespace}/fetch/{role}"))?;
                if has_commit(id)? {
                    break;
                }
            }
        }
        if !has_commit(id)? {
            return Err(GbError::DiffUnavailable {
                number: snapshot.number, cause: if role == "head" { "head_object_unavailable" } else { "base_object_unavailable" },
                message: format!("Cannot obtain saved {role} commit {}. Verify repository access and retained PR refs; current branch tips are not substitutes.", id.as_str()),
            });
        }
        let pin_identity = format!("{namespace}\0{}", id.as_str());
        let digest = ring::digest::digest(&ring::digest::SHA256, pin_identity.as_bytes());
        let digest: String = digest
            .as_ref()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        let pin = command()
            .args([
                "update-ref",
                &format!("refs/gb/pr-targets/v2/{digest}"),
                id.as_str(),
            ])
            .bounded_output()?;
        if !pin.status.success() {
            return Err(GbError::Other(
                "Cannot retain verified PR snapshot objects.".into(),
            ));
        }
    }
    Ok(())
}
