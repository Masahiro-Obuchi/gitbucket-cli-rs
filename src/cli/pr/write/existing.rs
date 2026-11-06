use crate::api::client::ApiClient;
use crate::error::{GbError, Result};
use crate::models::pull_request::PullRequest;

pub(super) async fn find_existing_open_pull_request(
    client: &ApiClient,
    owner: &str,
    repo: &str,
    head: &str,
    base: &str,
) -> Result<Option<PullRequest>> {
    // Enumerate completely before deciding absence. The pinned Issue collection
    // excludes PRs and cannot repair missing or failed PR pages.
    let prs = client.list_pull_requests(owner, repo, "open").await?;
    let branch = head.split_once(':').map_or(head, |(_, branch)| branch);
    for pr in prs {
        // Only branch refs are trustworthy in the 4.44 collection. It labels
        // fork heads as base-owned; ownership must not rule a candidate out.
        if pr
            .base
            .as_ref()
            .is_some_and(|value| !value.ref_name.is_empty() && value.ref_name != base)
            || pr
                .head
                .as_ref()
                .is_some_and(|value| !value.ref_name.is_empty() && value.ref_name != branch)
        {
            continue;
        }
        let actual = client
            .pull_request_details(owner, repo, pr.number)
            .await
            .map_err(|source| GbError::Context {
                source: Box::new(source),
                context: format!("Could not verify PR #{} before creating a PR", pr.number),
            })?;
        let actual = actual.ok_or_else(|| GbError::Other(format!("PR #{} has no canonical details. Cannot rule out an existing PR; no PR was created.", pr.number)))?;
        if actual.number != pr.number {
            return Err(GbError::Other(
                "Existing-PR lookup returned another number. No PR was created.".into(),
            ));
        }
        match compare(&actual, owner, repo, head, base) {
            Comparison::Match => return Ok(Some(actual)),
            Comparison::Different => {},
            Comparison::Unknown => return Err(GbError::Other(format!("PR #{} still lacks trustworthy head/base identity. Cannot rule out an existing open PR; no PR was created.", pr.number))),
        }
    }
    Ok(None)
}

enum Comparison {
    Match,
    Different,
    Unknown,
}

fn compare(pr: &PullRequest, owner: &str, repo: &str, head: &str, base: &str) -> Comparison {
    if pr.state == "closed" {
        return Comparison::Different;
    }
    if pr.state != "open" || pr.merged == Some(true) || pr.merged_at.is_some() {
        return Comparison::Unknown;
    }
    let (requested_owner, requested_branch) = head.split_once(':').unwrap_or((owner, head));
    let expected_base = format!("{owner}/{repo}");
    if pr
        .base
        .as_ref()
        .and_then(|base| base.repo.as_ref())
        .is_some_and(|repo| repo.full_name != expected_base)
    {
        return Comparison::Unknown;
    }
    if pr
        .base
        .as_ref()
        .is_some_and(|value| !value.ref_name.is_empty() && value.ref_name != base)
        || pr
            .head
            .as_ref()
            .is_some_and(|value| !value.ref_name.is_empty() && value.ref_name != requested_branch)
    {
        return Comparison::Different;
    }
    let (Some(actual_head), Some(actual_base)) = (&pr.head, &pr.base) else {
        return Comparison::Unknown;
    };
    if actual_head.ref_name.is_empty() || actual_base.ref_name.is_empty() {
        return Comparison::Unknown;
    }
    let identity = if let Some(repository) = &actual_head.repo {
        let Ok(name) = crate::target::RepositoryName::parse(&repository.full_name) else {
            return Comparison::Unknown;
        };
        if let Some(label) = &actual_head.label {
            let expected = format!("{}:{}", name.owner, actual_head.ref_name);
            if label != &actual_head.ref_name && label != &expected {
                return Comparison::Unknown;
            }
        }
        repository.full_name.clone()
    } else if let Some((label_owner, label_branch)) = actual_head
        .label
        .as_deref()
        .and_then(|label| label.split_once(':'))
    {
        if label_branch != actual_head.ref_name
            || crate::target::validate_component(label_owner).is_err()
        {
            return Comparison::Unknown;
        }
        format!("{label_owner}/{repo}")
    } else {
        return Comparison::Unknown;
    };
    if identity == format!("{requested_owner}/{repo}") {
        Comparison::Match
    } else {
        Comparison::Different
    }
}
