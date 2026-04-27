use colored::Colorize;

use crate::cli::common::{create_client, resolve_hostname, resolve_repo};
use crate::error::{GbError, Result};
use crate::models::pull_request::PullRequest;
use crate::output::table::format_table;
use crate::output::{format_state, page_or_print, truncate};

pub(super) struct ViewOptions {
    pub number: u64,
    pub show_comments: bool,
    pub web: bool,
    pub json: bool,
    pub no_pager: bool,
}

pub(super) async fn list(
    hostname: &Option<String>,
    cli_repo: &Option<String>,
    cli_profile: &Option<String>,
    state: &str,
    json: bool,
    no_pager: bool,
) -> Result<()> {
    let hostname = resolve_hostname(hostname, cli_profile)?;
    let (owner, repo) = resolve_repo(cli_repo, cli_profile)?;
    let client = create_client(&hostname, cli_profile)?;
    let state = crate::cli::common::normalize_list_state(state)?;

    let prs = client
        .list_repository_pull_requests(&owner, &repo, &state, json)
        .await?;

    if json {
        page_or_print(
            &format!("{}\n", serde_json::to_string_pretty(&prs)?),
            no_pager,
        )?;
        return Ok(());
    }

    let rows: Vec<Vec<String>> = prs
        .iter()
        .map(|pr| {
            let state = if pr.merged == Some(true) {
                "merged"
            } else {
                &pr.state
            };
            let branch = pr.head.as_ref().map(|h| h.ref_name.as_str()).unwrap_or("");
            vec![
                format!("#{}", pr.number),
                format_state(state),
                truncate(&pr.title, 50),
                branch.to_string(),
                pr.user
                    .as_ref()
                    .map(|u| u.login.clone())
                    .unwrap_or_default(),
            ]
        })
        .collect();

    page_or_print(
        &format_table(&["#", "STATE", "TITLE", "BRANCH", "AUTHOR"], &rows),
        no_pager,
    )?;
    Ok(())
}

pub(super) async fn list_comments(
    hostname: &Option<String>,
    cli_repo: &Option<String>,
    cli_profile: &Option<String>,
    number: u64,
    json: bool,
    no_pager: bool,
) -> Result<()> {
    let hostname = resolve_hostname(hostname, cli_profile)?;
    let (owner, repo) = resolve_repo(cli_repo, cli_profile)?;
    let client = create_client(&hostname, cli_profile)?;

    let comments = client.list_all_pr_comments(&owner, &repo, number).await?;

    if json {
        page_or_print(
            &format!("{}\n", serde_json::to_string_pretty(&comments)?),
            no_pager,
        )?;
        return Ok(());
    }

    let rows: Vec<Vec<String>> = comments
        .iter()
        .map(|comment| {
            vec![
                comment.id.to_string(),
                comment
                    .user
                    .as_ref()
                    .map(|user| user.login.clone())
                    .unwrap_or_default(),
                comment.created_at.clone().unwrap_or_default(),
                {
                    let raw = comment.body.as_deref().unwrap_or("");
                    let normalized = raw.replace(['\r', '\n'], " ");
                    let collapsed: String =
                        normalized.split_whitespace().collect::<Vec<_>>().join(" ");
                    truncate(&collapsed, 70)
                },
            ]
        })
        .collect();

    page_or_print(
        &format_table(&["ID", "AUTHOR", "CREATED", "BODY"], &rows),
        no_pager,
    )?;
    Ok(())
}

pub(super) async fn view(
    hostname: &Option<String>,
    cli_repo: &Option<String>,
    cli_profile: &Option<String>,
    options: ViewOptions,
) -> Result<()> {
    let hostname = resolve_hostname(hostname, cli_profile)?;
    let (owner, repo) = resolve_repo(cli_repo, cli_profile)?;
    let client = create_client(&hostname, cli_profile)?;

    if options.web {
        let url = client.web_url(&format!("/{}/{}/pull/{}", owner, repo, options.number));
        open::that(&url).map_err(|e| GbError::Other(format!("Failed to open browser: {}", e)))?;
        println!("Opening {} in your browser.", url);
        return Ok(());
    }

    let pr = client
        .get_pull_request(&owner, &repo, options.number)
        .await?;

    if options.json {
        if options.show_comments {
            let comments = client
                .list_pr_comments(&owner, &repo, options.number)
                .await?;
            let mut value = serde_json::to_value(&pr)?;
            let object = value.as_object_mut().ok_or_else(|| {
                GbError::Other("Failed to serialize pull request as JSON object".into())
            })?;
            object.insert("comments".into(), serde_json::to_value(comments)?);
            page_or_print(
                &format!("{}\n", serde_json::to_string_pretty(&value)?),
                options.no_pager,
            )?;
        } else {
            page_or_print(
                &format!("{}\n", serde_json::to_string_pretty(&pr)?),
                options.no_pager,
            )?;
        }
        return Ok(());
    }

    let output = format_pr_view(
        &client,
        &owner,
        &repo,
        options.number,
        options.show_comments,
        &pr,
    )
    .await?;
    page_or_print(&output, options.no_pager)?;
    Ok(())
}

async fn format_pr_view(
    client: &crate::api::client::ApiClient,
    owner: &str,
    repo: &str,
    number: u64,
    show_comments: bool,
    pr: &PullRequest,
) -> Result<String> {
    let state = if pr.merged == Some(true) {
        "merged"
    } else {
        &pr.state
    };
    let mut output = String::new();
    output.push_str(&format!(
        "{} {}\n",
        pr.title.bold(),
        format!("#{}", pr.number).dimmed()
    ));
    output.push_str(&format!("{}\n\n", format_state(state)));

    if let Some(head) = &pr.head {
        if let Some(base) = &pr.base {
            output.push_str(&format!(
                "{} ← {}\n",
                base.ref_name.cyan(),
                head.ref_name.green()
            ));
        }
    }

    if let Some(user) = &pr.user {
        output.push_str(&format!("Author: {}  ", user.login));
    }
    if let Some(created) = &pr.created_at {
        output.push_str(&format!("Created: {}", created));
    }
    output.push('\n');

    if let Some(body) = &pr.body {
        if !body.is_empty() {
            output.push_str(&format!("\n{}\n", body));
        }
    }

    if show_comments {
        let comments = client.list_pr_comments(owner, repo, number).await?;
        if !comments.is_empty() {
            output.push_str(&format!("\n{}\n", "--- Comments ---".dimmed()));
            for c in &comments {
                let author = c
                    .user
                    .as_ref()
                    .map(|u| u.login.as_str())
                    .unwrap_or("unknown");
                let date = c.created_at.as_deref().unwrap_or("");
                output.push_str(&format!("\n{} ({})\n", author.bold(), date.dimmed()));
                if let Some(body) = &c.body {
                    output.push_str(body);
                    output.push('\n');
                }
            }
        }
    }

    Ok(output)
}

pub(super) fn print_pr_refs(pr: &PullRequest) {
    if let Some(head) = &pr.head {
        println!("Head: {}", format_pr_ref(head));
    }
    if let Some(base) = &pr.base {
        println!("Base: {}", format_pr_ref(base));
    }
}

fn format_pr_ref(pr_ref: &crate::models::pull_request::PullRequestHead) -> String {
    match &pr_ref.repo {
        Some(repo) => format!("{}:{}", repo.full_name, pr_ref.ref_name),
        None => pr_ref
            .label
            .clone()
            .unwrap_or_else(|| pr_ref.ref_name.clone()),
    }
}
