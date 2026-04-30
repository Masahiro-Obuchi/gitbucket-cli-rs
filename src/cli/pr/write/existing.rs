use std::collections::HashSet;

use crate::error::{GbError, Result};
use crate::models::pull_request::PullRequest;

pub(super) async fn find_existing_open_pull_request(
    client: &crate::api::client::ApiClient,
    owner: &str,
    repo: &str,
    head: &str,
    base: &str,
) -> Result<Option<PullRequest>> {
    let prs = client.list_pull_requests(owner, repo, "open").await?;
    let mut seen: HashSet<u64> = prs.iter().map(|pr| pr.number).collect();

    if let Some(pr) =
        find_matching_pull_request(client, owner, repo, head, base, prs.into_iter()).await?
    {
        return Ok(Some(pr));
    }

    let issues = match client.list_issues(owner, repo, "open").await {
        Ok(issues) => issues,
        Err(GbError::Api { status, .. }) if status == 404 || status == 501 => return Ok(None),
        Err(err) => {
            eprintln!(
                "Notice: skipping issue-list fallback while checking for an existing PR: {}",
                err
            );
            return Ok(None);
        }
    };

    for issue in issues {
        if issue.pull_request.is_none() || !seen.insert(issue.number) {
            continue;
        }

        match client.get_pull_request(owner, repo, issue.number).await {
            Ok(pr) if pull_request_matches_head_base(&pr, owner, repo, head, base) => {
                return Ok(Some(pr));
            }
            Ok(_) => {}
            Err(GbError::Api { status, .. }) if status == 404 || status == 501 => {}
            Err(err) => {
                eprintln!(
                    "Notice: skipping pull request #{} while checking for an existing PR: {}",
                    issue.number, err
                );
            }
        }
    }

    Ok(None)
}

async fn find_matching_pull_request(
    client: &crate::api::client::ApiClient,
    owner: &str,
    repo: &str,
    head: &str,
    base: &str,
    prs: impl Iterator<Item = PullRequest>,
) -> Result<Option<PullRequest>> {
    for pr in prs {
        if pull_request_matches_head_base(&pr, owner, repo, head, base) {
            return Ok(Some(pr));
        }

        if !pull_request_has_comparable_refs(&pr) {
            match client.get_pull_request(owner, repo, pr.number).await {
                Ok(pr) if pull_request_matches_head_base(&pr, owner, repo, head, base) => {
                    return Ok(Some(pr));
                }
                Ok(_) => {}
                Err(GbError::Api { status, .. }) if status == 404 || status == 501 => {}
                Err(err) => {
                    eprintln!(
                        "Notice: skipping pull request #{} while checking for an existing PR: {}",
                        pr.number, err
                    );
                }
            }
        }
    }

    Ok(None)
}

fn pull_request_has_comparable_refs(pr: &PullRequest) -> bool {
    pr.head.as_ref().is_some_and(|head| {
        !head.ref_name.is_empty()
            && (head.label.as_deref().is_some_and(|label| !label.is_empty())
                || head
                    .repo
                    .as_ref()
                    .is_some_and(|repo| !repo.full_name.is_empty()))
    }) && pr
        .base
        .as_ref()
        .is_some_and(|base| !base.ref_name.is_empty())
}

fn pull_request_matches_head_base(
    pr: &PullRequest,
    owner: &str,
    repo: &str,
    head: &str,
    base: &str,
) -> bool {
    if pr
        .base
        .as_ref()
        .is_none_or(|pr_base| pr_base.ref_name != base)
    {
        return false;
    }

    let Some(pr_head) = pr.head.as_ref() else {
        return false;
    };

    let (head_owner, head_branch) = head
        .split_once(':')
        .map(|(owner, branch)| (Some(owner), branch))
        .unwrap_or((None, head));
    if pr_head.ref_name != head_branch {
        return false;
    }

    match head_owner {
        Some(head_owner) => {
            pr_head.label.as_deref() == Some(head)
                || pr_head
                    .repo
                    .as_ref()
                    .is_some_and(|head_repo| head_repo.full_name == format!("{head_owner}/{repo}"))
        }
        None => {
            pr_head.label.as_deref() == Some(head)
                || pr_head.label.as_deref() == Some(&format!("{owner}:{head_branch}"))
                || pr_head
                    .repo
                    .as_ref()
                    .is_some_and(|head_repo| head_repo.full_name == format!("{owner}/{repo}"))
        }
    }
}
