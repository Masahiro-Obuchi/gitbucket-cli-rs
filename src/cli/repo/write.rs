use dialoguer::{Confirm, Input};

use crate::api::client::ApiClient;
use crate::cli::common::{create_web_session, parse_owner_repo, HostContext, RepoContext};
use crate::error::{GbError, Result};
use crate::models::repository::{CreateRepository, Repository};

pub(super) async fn create(
    hostname: &Option<String>,
    cli_profile: &Option<String>,
    name: Option<String>,
    description: Option<String>,
    private: bool,
    add_readme: bool,
    group: Option<String>,
) -> Result<()> {
    let ctx = HostContext::resolve(hostname, cli_profile)?;

    let name = match name {
        Some(n) => n,
        None => {
            crate::cancel::input(|| Input::new().with_prompt("Repository name").interact_text())
                .await?
        }
    };

    let body = CreateRepository {
        name: name.clone(),
        description,
        is_private: Some(private),
        auto_init: Some(add_readme),
    };

    let repo = match group {
        Some(group_name) => ctx.client.create_org_repo(&group_name, &body).await?,
        None => ctx.client.create_user_repo(&body).await?,
    };

    println!("✓ Created repository {}", repo.full_name);
    if let Some(url) = &repo.html_url {
        println!("{}", url);
    }
    Ok(())
}

pub(super) async fn delete(
    hostname: &Option<String>,
    cli_profile: &Option<String>,
    repo_arg: Option<String>,
    yes: bool,
) -> Result<()> {
    let repo_arg = repo_arg.ok_or_else(|| {
        GbError::Other(
            "Refusing to delete without an explicit repository. Pass OWNER/REPO or -R/--repo."
                .into(),
        )
    })?;
    let (owner, repo) = parse_owner_repo(&repo_arg)?;

    if !yes {
        let prompt = format!("Are you sure you want to delete {}/{}?", owner, repo);
        let confirmed = crate::cancel::input(move || {
            Confirm::new().with_prompt(prompt).default(false).interact()
        })
        .await?;
        if !confirmed {
            println!("Aborted.");
            return Ok(());
        }
    }

    let ctx = HostContext::resolve(hostname, cli_profile)?;
    // Establish existence/read permission before interpreting DELETE 404 as a
    // missing route. A missing/private target must never trigger Web deletion.
    let before = ctx.client.get_repo(&owner, &repo).await?;
    if before.full_name != format!("{owner}/{repo}") {
        return Err(GbError::Other(
            "Repository lookup returned another target.".into(),
        ));
    }
    let mut progress = crate::operation::Progress::default();
    let result = ctx.client.delete_repo(&owner, &repo).await;
    match result {
        Err(GbError::Api { status: 404, .. }) => {
            let step = progress.plan("repository.delete", crate::operation::Route::Web);
            let session = create_web_session(&ctx.client, &ctx.hostname, cli_profile)
                .await
                .map_err(|err| progress.error(err))?;
            progress.actor(session.principal());
            progress
                .run(step, session.delete_repo(&owner, &repo), |_| {
                    verify_deleted(&ctx.client, &owner, &repo)
                })
                .await?;
        }
        result => {
            let step = progress.plan("repository.delete", crate::operation::Route::Rest);
            progress
                .run(step, async { result }, |_| {
                    verify_deleted(&ctx.client, &owner, &repo)
                })
                .await?;
        }
    }
    println!("✓ Deleted repository {}/{}", owner, repo);
    Ok(())
}

async fn verify_deleted(client: &ApiClient, owner: &str, repo: &str) -> Result<()> {
    match client.get_repo(owner, repo).await {
        Err(GbError::Api { status: 404, .. }) => Ok(()),
        Err(err) => Err(err),
        Ok(_) => Err(GbError::Other(
            "Repository delete was submitted, but the repository still exists.".into(),
        )),
    }
}

pub(super) async fn fork(
    hostname: &Option<String>,
    repo_arg: Option<String>,
    cli_profile: &Option<String>,
    group: Option<String>,
) -> Result<()> {
    let ctx = RepoContext::resolve(hostname, &repo_arg, cli_profile)?;
    if let Some(group) = &group {
        crate::target::validate_component(group)?;
    }
    let api_user = ctx.client.current_user().await?;
    let target = group.as_deref().unwrap_or(&api_user.login);
    crate::target::validate_component(target)?;
    let source = ctx.client.get_repo(&ctx.owner, &ctx.repo).await?;
    if source.full_name != format!("{}/{}", ctx.owner, ctx.repo) {
        return Err(GbError::Other(
            "Source lookup returned another repository.".into(),
        ));
    }
    // A pre-existing unrelated destination is never a successful fork.
    if let Some(existing) = existing_fork(&ctx, cli_profile, target).await? {
        print_fork_result(&ctx.owner, &ctx.repo, &existing);
        return Ok(());
    }
    let mut progress = crate::operation::Progress::default();
    let result = ctx
        .client
        .fork_repo(&ctx.owner, &ctx.repo, group.as_deref())
        .await;
    let forked = match result {
        Err(GbError::Api { status: 404, .. }) => {
            // Confirm the source is still readable; 404 alone is not evidence
            // that an endpoint is unavailable.
            ctx.client.get_repo(&ctx.owner, &ctx.repo).await?;
            let step = progress.plan("repository.fork", crate::operation::Route::Web);
            let session = create_web_session(&ctx.client, &ctx.hostname, cli_profile)
                .await
                .map_err(|err| progress.error(err))?;
            if group.is_none() && !session.principal().eq_ignore_ascii_case(target) {
                return Err(progress.error(GbError::Auth(
                    "A fork for another Web actor requires an explicit --group destination.".into(),
                )));
            }
            progress.actor(session.principal());
            progress
                .run(
                    step,
                    session.fork_repo(&ctx.owner, &ctx.repo, target),
                    |_| verify_fork(&ctx.client, target, &ctx.repo, &ctx.owner, Some(&session)),
                )
                .await?
        }
        result => {
            let step = progress.plan("repository.fork", crate::operation::Route::Rest);
            let ctx = &ctx;
            progress
                .run(step, async { result }, |returned| async move {
                    if !repository_is_requested_fork(
                        &returned, target, &ctx.repo, &ctx.owner, &ctx.repo,
                    ) && (returned.parent.is_some()
                        || returned.full_name != format!("{target}/{}", ctx.repo))
                    {
                        return Err(GbError::Other(
                            "Fork response does not match the destination and immediate upstream."
                                .into(),
                        ));
                    }
                    verify_fork(&ctx.client, target, &ctx.repo, &ctx.owner, None).await
                })
                .await?
        }
    };
    print_fork_result(&ctx.owner, &ctx.repo, &forked);
    Ok(())
}

async fn existing_fork(
    ctx: &RepoContext,
    cli_profile: &Option<String>,
    target: &str,
) -> Result<Option<Repository>> {
    let (client, repo, owner) = (&ctx.client, ctx.repo.as_str(), ctx.owner.as_str());
    match client.get_repo(target, repo).await {
        Ok(existing) if repository_is_requested_fork(&existing, target, repo, owner, repo) => {
            Ok(Some(existing))
        }
        Ok(existing)
            if existing.parent.is_none() && existing.full_name == format!("{target}/{repo}") =>
        {
            if existing.is_private {
                let session = create_web_session(client, &ctx.hostname, cli_profile).await?;
                verify_parent(client, Some(&session), target, repo, owner).await?;
            } else {
                verify_parent(client, None, target, repo, owner).await?;
            }
            Ok(Some(existing))
        }
        Ok(_) => Err(GbError::Other(
            "The fork destination already exists with a different upstream. No fork was attempted."
                .into(),
        )),
        Err(GbError::Api { status: 404, .. }) => Ok(None),
        Err(err) => Err(err),
    }
}

async fn verify_fork(
    client: &ApiClient,
    target: &str,
    repo: &str,
    owner: &str,
    session: Option<&crate::api::web::GitBucketWebSession>,
) -> Result<Repository> {
    // The pinned Web controller starts a background fork. Poll reads only;
    // never repeat the mutation when creation is still in progress.
    let mut attempts = 0;
    let stored = loop {
        match client.get_repo(target, repo).await {
            Err(GbError::Api { status: 404, .. }) if attempts < 20 => {
                attempts += 1;
                tokio::time::sleep(std::time::Duration::from_millis(250)).await;
            }
            result => break result?,
        }
    };
    if stored.full_name != format!("{target}/{repo}") || stored.name != repo {
        return Err(GbError::Other(
            "Stored fork destination does not match.".into(),
        ));
    }
    if stored.parent.is_none() {
        verify_parent(client, session, target, repo, owner).await?;
    } else if !repository_is_requested_fork(&stored, target, repo, owner, repo) {
        return Err(GbError::Other(
            "Fork was submitted, but stored destination or immediate upstream does not match."
                .into(),
        ));
    }
    Ok(stored)
}

async fn verify_parent(
    client: &ApiClient,
    session: Option<&crate::api::web::GitBucketWebSession>,
    target: &str,
    repo: &str,
    owner: &str,
) -> Result<()> {
    let target = format!("{target}/{repo}");
    let source = format!("{owner}/{repo}");
    if let Some(session) = session {
        return session.verify_fork_parent(&target, &source).await;
    }
    // Read-only public metadata does not need a Web password. If a private or
    // unfamiliar page cannot establish the parent, report it as unverified.
    let page_url = client.web_url(&format!("/{target}"));
    let page = client.get_text_from_origin(&page_url).await?;
    crate::api::web::verify_parent_html(&page, &client.web_url(""), &target, &source)
}

fn repository_is_requested_fork(
    repo: &Repository,
    target_owner: &str,
    target_repo: &str,
    source_owner: &str,
    source_repo: &str,
) -> bool {
    if !repo.fork || repo.full_name != format!("{target_owner}/{target_repo}") {
        return false;
    }

    let source_full_name = format!("{source_owner}/{source_repo}");
    repo.parent
        .as_ref()
        .is_some_and(|parent| parent.full_name == source_full_name)
}

fn print_fork_result(owner: &str, repo: &str, forked: &Repository) {
    println!("✓ Forked {}/{} → {}", owner, repo, forked.full_name);
    if let Some(url) = &forked.html_url {
        println!("{}", url);
    }
}
