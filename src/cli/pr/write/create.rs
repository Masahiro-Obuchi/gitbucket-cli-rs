use crate::error::{GbError, Result};
use crate::models::pull_request::{CreatePullRequest, PullRequest};
use crate::{cli::common::RepoContext, output};

use super::super::git::current_branch_name;
use super::super::read::print_pr_refs;
use super::existing::find_existing_open_pull_request;

pub(in crate::cli::pr) struct CreateRequest {
    pub title: Option<String>,
    pub body: Option<String>,
    pub head: Option<String>,
    pub head_owner: Option<String>,
    pub base: Option<String>,
    pub json: bool,
    pub detect_existing: bool,
}

pub(in crate::cli::pr) async fn create(
    hostname: &Option<String>,
    cli_repo: &Option<String>,
    cli_profile: &Option<String>,
    request: CreateRequest,
) -> Result<()> {
    let ctx = RepoContext::resolve(hostname, cli_repo, cli_profile)?;
    if request.title.is_none() && !request.detect_existing && !crate::cli::input::interactive() {
        return Err(crate::cli::input::required("--title"));
    }
    let inferred_head = request.head.is_none();

    let head_branch = match request.head {
        Some(h) => h,
        None => current_branch_name()?.ok_or_else(|| {
            GbError::Other(
                "Could not determine current branch. Specify --head when running from a detached HEAD state.".into(),
            )
        })?,
    };
    let head_owner = request
        .head_owner
        .as_deref()
        .unwrap_or(&ctx.owner)
        .trim()
        .to_owned();
    let head = qualified_head(head_branch.clone(), request.head_owner)?;

    let target_repo = if request.base.is_none() || (inferred_head && head_owner == ctx.owner) {
        let repo = ctx.client.get_repo(&ctx.owner, &ctx.repo).await?;
        if repo.full_name != format!("{}/{}", ctx.owner, ctx.repo) {
            return Err(GbError::Other(
                "Repository lookup returned a different PR target.".into(),
            ));
        }
        Some(repo)
    } else {
        None
    };
    let base = match request.base {
        Some(b) => b,
        None => target_repo
            .as_ref()
            .and_then(|repo| repo.default_branch.clone())
            .filter(|name| !name.trim().is_empty())
            .ok_or_else(|| {
                crate::cli::input::required("--base (the server did not provide a default branch)")
            })?,
    };

    if request.detect_existing {
        if let Some(pr) =
            find_existing_open_pull_request(&ctx.client, &ctx.owner, &ctx.repo, &head, &base)
                .await?
        {
            print_pr_create_result(
                &ctx.client,
                &ctx.owner,
                &ctx.repo,
                &pr,
                request.json,
                "Found existing",
            )?;
            return Ok(());
        }
    }

    let title = crate::cli::input::title(request.title).await?;
    let body_text = crate::cli::input::optional_body(request.body).await?;
    if inferred_head {
        let repo = if head_owner == ctx.owner {
            target_repo.expect("inferred head resolves the target repository")
        } else {
            let repo = ctx.client.get_repo(&head_owner, &ctx.repo).await?;
            if repo.full_name != format!("{head_owner}/{}", ctx.repo) {
                return Err(GbError::Other(
                    "Repository lookup returned a different head repository.".into(),
                ));
            }
            repo
        };
        let (sha, dirty) = crate::git::published::verify(&ctx.git_auth, &repo, &head_branch)?;
        eprintln!("Notice: Local HEAD matches published {head} at {sha}; base is {base}.");
        if dirty {
            eprintln!(
                "Notice: Uncommitted changes are excluded from the PR. No push was attempted."
            );
        }
    }

    let create_body = CreatePullRequest {
        title,
        head: head.clone(),
        base: base.clone(),
        body: body_text,
    };

    match ctx
        .client
        .create_pull_request(&ctx.owner, &ctx.repo, &create_body)
        .await
    {
        Ok(pr) => print_pr_create_result(
            &ctx.client,
            &ctx.owner,
            &ctx.repo,
            &pr,
            request.json,
            "Created",
        ),
        Err(err)
            if request.detect_existing
                && err.code() != "cancelled"
                && !crate::cancel::requested() =>
        {
            match find_existing_open_pull_request(&ctx.client, &ctx.owner, &ctx.repo, &head, &base)
                .await
            {
                Ok(Some(pr)) => {
                    eprintln!("Notice: PR create failed; returning an existing open PR.");
                    print_pr_create_result(
                        &ctx.client,
                        &ctx.owner,
                        &ctx.repo,
                        &pr,
                        request.json,
                        "Found existing",
                    )
                }
                Ok(None) => Err(err),
                Err(check) if check.code() == "cancelled" || crate::cancel::requested() => Err(GbError::Context {
                    source: Box::new(err),
                    context: "The existing-PR recheck was cancelled after PR creation failed. Inspect the repository before retrying".into(),
                }),
                Err(check) => Err(GbError::Context { source: Box::new(err), context: format!("PR creation failed and a complete existing-PR recheck also failed ({check}). Inspect the repository before retrying") }),
            }
        }
        Err(err) => Err(err),
    }
}

fn qualified_head(head: String, head_owner: Option<String>) -> Result<String> {
    let Some(owner) = head_owner else {
        return Ok(head);
    };
    if head.contains(':') {
        return Err(GbError::Other(
            "Cannot use --head-owner when --head is already qualified as OWNER:BRANCH.".into(),
        ));
    }
    let owner = owner.trim();
    if owner.is_empty() {
        return Err(GbError::Other("--head-owner cannot be empty.".into()));
    }
    if owner.contains(':') {
        return Err(GbError::Other(
            "--head-owner cannot contain ':'. Expected an unqualified owner name.".into(),
        ));
    }
    Ok(format!("{owner}:{head}"))
}

fn print_pr_create_result(
    client: &crate::api::client::ApiClient,
    owner: &str,
    repo: &str,
    pr: &PullRequest,
    json: bool,
    verb: &str,
) -> Result<()> {
    if json {
        return output::print_json(pr);
    }

    println!(
        "✓ {} pull request #{}: {}",
        verb,
        pr.number,
        crate::output::single_line(&pr.title)
    );
    print_pr_refs(pr);
    println!("URL: {}", pr_url(client, owner, repo, pr));
    Ok(())
}

fn pr_url(
    client: &crate::api::client::ApiClient,
    owner: &str,
    repo: &str,
    pr: &PullRequest,
) -> String {
    pr.html_url
        .as_deref()
        .filter(|url| !url.contains("/pulls/"))
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| client.web_url(&format!("/{owner}/{repo}/pull/{}", pr.number)))
}

#[cfg(test)]
mod tests {
    use super::qualified_head;

    #[test]
    fn qualified_head_uses_owner_prefix() {
        assert_eq!(
            qualified_head("feature".into(), Some("alice".into())).unwrap(),
            "alice:feature"
        );
    }

    #[test]
    fn qualified_head_rejects_duplicate_owner_syntax() {
        assert!(qualified_head("alice:feature".into(), Some("bob".into())).is_err());
    }
}
