use dialoguer::Input;

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

    let head_branch = match request.head {
        Some(h) => h,
        None => current_branch_name().ok_or_else(|| {
            GbError::Other(
                "Could not determine current branch. Specify --head when running from a detached HEAD state.".into(),
            )
        })?,
    };
    let head = qualified_head(head_branch, request.head_owner)?;

    let base = match request.base {
        Some(b) => b,
        None => Input::new()
            .with_prompt("Base branch")
            .default("main".to_string())
            .interact_text()?,
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

    let title = match request.title {
        Some(t) => t,
        None => Input::new().with_prompt("Title").interact_text()?,
    };

    let body_text = match request.body {
        Some(b) => Some(b),
        None => {
            let b: String = Input::new()
                .with_prompt("Body (optional)")
                .allow_empty(true)
                .interact_text()?;
            if b.is_empty() {
                None
            } else {
                Some(b)
            }
        }
    };

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
        Err(err) if request.detect_existing => {
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
                Ok(None) | Err(_) => Err(err),
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

    println!("✓ {} pull request #{}: {}", verb, pr.number, pr.title);
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
