//! Check an inferred local PR head against the live published ref, without fetching/pushing.
use super::{GitAuth, GitCommandExt};
use crate::{
    domain::pull_request::CommitId,
    error::{GbError, Result},
    models::repository::Repository,
};

pub(crate) fn verify(auth: &GitAuth, repo: &Repository, branch: &str) -> Result<(String, bool)> {
    let reference = format!("refs/heads/{branch}");
    if !super::snapshot::command()
        .args(["check-ref-format", &reference])
        .bounded_output()?
        .status
        .success()
    {
        return Err(GbError::Other(
            "Invalid inferred head branch. Specify --head explicitly.".into(),
        ));
    }
    let local = super::snapshot::command()
        .args(["rev-parse", "--verify", "--end-of-options", "HEAD^{commit}"])
        .bounded_output()?;
    let local = if local.status.success() {
        CommitId::parse(String::from_utf8_lossy(&local.stdout).trim())?
    } else {
        return Err(GbError::Other(
            "Cannot read local HEAD. Specify --head explicitly.".into(),
        ));
    };
    let source = super::clean_source(&auth.clone_url(repo))?;
    let mut remote = auth
        .command(&source)?
        .args(["ls-remote", "--heads", "--", &source, &reference])
        .bounded_output()?;
    auth.redact_output(&mut remote);
    if !remote.status.success() {
        return Err(GbError::Other("Cannot verify the published head branch. No PR was created. Check Git access, or specify --head explicitly to use a published branch.".into()));
    }
    let output = String::from_utf8_lossy(&remote.stdout);
    let mut refs = output
        .lines()
        .filter_map(|line| line.split_once('\t'))
        .filter(|(_, name)| *name == reference);
    let sha = refs.next().map(|(sha, _)| sha);
    if refs.next().is_some() || sha != Some(local.as_str()) {
        return Err(GbError::Other("The inferred local HEAD is not the published head (missing, ahead, behind, or diverged). No PR was created and no push was attempted. Push the intended branch yourself, or specify --head explicitly to use the published branch.".into()));
    }
    let status = super::snapshot::command()
        .env("GIT_OPTIONAL_LOCKS", "0")
        .args(["status", "--porcelain", "--untracked-files=normal"])
        .bounded_output()?;
    if !status.status.success() {
        return Err(GbError::Other(
            "Cannot inspect local working tree before PR creation.".into(),
        ));
    }
    Ok((local.as_str().to_owned(), !status.stdout.is_empty()))
}
