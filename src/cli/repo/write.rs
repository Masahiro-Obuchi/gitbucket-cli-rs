use dialoguer::{Confirm, Input};

use crate::api::client::ApiClient;
use crate::cli::common::{
    create_web_session, parse_owner_repo, resolve_host_config, resolve_repo, HostContext,
};
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
        None => Input::new()
            .with_prompt("Repository name")
            .interact_text()?,
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
        let confirmed = Confirm::new()
            .with_prompt(format!(
                "Are you sure you want to delete {}/{}?",
                owner, repo
            ))
            .default(false)
            .interact()?;
        if !confirmed {
            println!("Aborted.");
            return Ok(());
        }
    }

    let ctx = HostContext::resolve(hostname, cli_profile)?;
    match ctx.client.delete_repo(&owner, &repo).await {
        Ok(()) => {
            println!("✓ Deleted repository {}/{}", owner, repo);
            Ok(())
        }
        Err(GbError::Api { status: 404, .. }) => {
            eprintln!(
                "Notice: REST repository delete is unavailable on this GitBucket instance; using web fallback."
            );
            let session = create_web_session(&ctx.hostname, cli_profile).await?;
            session.delete_repo(&owner, &repo).await?;
            println!("✓ Deleted repository {}/{}", owner, repo);
            Ok(())
        }
        Err(err) => Err(err),
    }
}

pub(super) async fn fork(
    hostname: &Option<String>,
    repo_arg: Option<String>,
    cli_profile: &Option<String>,
    group: Option<String>,
) -> Result<()> {
    let (owner, repo) = match repo_arg {
        Some(r) => parse_owner_repo(&r)?,
        None => resolve_repo(&None, cli_profile)?,
    };

    let ctx = HostContext::resolve(hostname, cli_profile)?;
    match ctx.client.fork_repo(&owner, &repo).await {
        Ok(forked) => {
            print_fork_result(&owner, &repo, &forked);
            Ok(())
        }
        Err(err @ GbError::Api { status, .. }) => {
            if status != 404 {
                if status == 409 || status >= 500 {
                    if let Ok(target_account) =
                        resolve_fork_target(&ctx.hostname, cli_profile, group)
                    {
                        if let Some(existing) =
                            existing_fork(&ctx.client, &target_account, &repo, &owner, &repo)
                                .await?
                        {
                            eprintln!(
                                "Notice: fork request did not return a repository; using existing fork {}.",
                                existing.full_name
                            );
                            print_fork_result(&owner, &repo, &existing);
                            return Ok(());
                        }
                    }
                }
                return Err(err);
            }

            let target_account = resolve_fork_target(&ctx.hostname, cli_profile, group)?;
            if let Some(existing) =
                existing_fork(&ctx.client, &target_account, &repo, &owner, &repo).await?
            {
                eprintln!(
                    "Notice: fork request did not return a repository; using existing fork {}.",
                    existing.full_name
                );
                print_fork_result(&owner, &repo, &existing);
                return Ok(());
            }

            eprintln!(
                "Notice: REST repository fork is unavailable on this GitBucket instance; using web fallback."
            );
            let session = create_web_session(&ctx.hostname, cli_profile).await?;
            session.fork_repo(&owner, &repo, &target_account).await?;
            println!("✓ Forked {}/{} → {}/{}", owner, repo, target_account, repo);
            println!(
                "{}",
                ctx.client.web_url(&format!("/{}/{}", target_account, repo))
            );
            Ok(())
        }
        Err(err) => Err(err),
    }
}

async fn existing_fork(
    client: &ApiClient,
    target_owner: &str,
    target_repo: &str,
    source_owner: &str,
    source_repo: &str,
) -> Result<Option<Repository>> {
    match client.get_repo(target_owner, target_repo).await {
        Ok(repo)
            if repository_is_requested_fork(
                &repo,
                target_owner,
                target_repo,
                source_owner,
                source_repo,
            ) =>
        {
            Ok(Some(repo))
        }
        Ok(repo) => {
            if repo.fork && repo.full_name == format!("{target_owner}/{target_repo}") {
                eprintln!(
                    "Notice: found existing fork {}, but its upstream did not match {}/{}.",
                    repo.full_name, source_owner, source_repo
                );
            }
            Ok(None)
        }
        Err(GbError::Api { status: 404, .. }) => Ok(None),
        Err(err) => {
            eprintln!(
                "Notice: failed to check existing fork {}/{} after fork error for {}/{}: {}",
                target_owner, target_repo, source_owner, source_repo, err
            );
            Ok(None)
        }
    }
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
        || repo
            .source
            .as_ref()
            .is_some_and(|source| source.full_name == source_full_name)
}

fn print_fork_result(owner: &str, repo: &str, forked: &Repository) {
    println!("✓ Forked {}/{} → {}", owner, repo, forked.full_name);
    if let Some(url) = &forked.html_url {
        println!("{}", url);
    }
}

fn resolve_fork_target(
    hostname: &str,
    cli_profile: &Option<String>,
    group: Option<String>,
) -> Result<String> {
    if let Some(group) = group {
        return Ok(group);
    }
    if let Ok(user) = std::env::var("GB_USER") {
        if !user.is_empty() {
            return Ok(user);
        }
    }

    let host = resolve_host_config(hostname, cli_profile)?;
    if !host.user.is_empty() {
        return Ok(host.user);
    }

    Err(GbError::Auth(
        "GitBucket fork requires a destination user or group. Run `gb auth login` first, pass `--group`, or set `GB_USER`."
            .into(),
    ))
}
