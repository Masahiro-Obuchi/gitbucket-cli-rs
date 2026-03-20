use clap::{Args, Subcommand};
use colored::Colorize;
use dialoguer::Input;

use crate::cli::common::{create_client, resolve_hostname, resolve_repo};
use crate::error::Result;
use crate::models::comment::CreateComment;
use crate::models::pull_request::{CreatePullRequest, MergePullRequest};
use crate::output::table::print_table;
use crate::output::{format_state, truncate};

#[derive(Args)]
pub struct PrArgs {
    #[command(subcommand)]
    pub command: PrCommand,
}

#[derive(Subcommand)]
pub enum PrCommand {
    /// List pull requests
    List {
        /// Filter by state (open, closed, all)
        #[arg(long, short, default_value = "open")]
        state: String,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
    /// View a pull request
    View {
        /// PR number
        number: u64,
        /// Show comments
        #[arg(long, short)]
        comments: bool,
        /// Open in browser
        #[arg(long, short)]
        web: bool,
    },
    /// Create a pull request
    Create {
        /// PR title
        #[arg(long, short)]
        title: Option<String>,
        /// PR body
        #[arg(long, short)]
        body: Option<String>,
        /// Head branch
        #[arg(long, short = 'H')]
        head: Option<String>,
        /// Base branch
        #[arg(long, short = 'B')]
        base: Option<String>,
    },
    /// Close a pull request
    Close {
        /// PR number
        number: u64,
    },
    /// Merge a pull request
    Merge {
        /// PR number
        number: u64,
        /// Merge commit message
        #[arg(long, short)]
        message: Option<String>,
    },
    /// Checkout a pull request branch locally
    Checkout {
        /// PR number
        number: u64,
    },
    /// View the diff of a pull request
    Diff {
        /// PR number
        number: u64,
    },
    /// Add a comment to a pull request
    Comment {
        /// PR number
        number: u64,
        /// Comment body
        #[arg(long, short)]
        body: Option<String>,
    },
}

pub async fn run(
    args: PrArgs,
    cli_hostname: &Option<String>,
    cli_repo: &Option<String>,
) -> Result<()> {
    match args.command {
        PrCommand::List { state, json } => list(cli_hostname, cli_repo, &state, json).await,
        PrCommand::View {
            number,
            comments,
            web,
        } => view(cli_hostname, cli_repo, number, comments, web).await,
        PrCommand::Create {
            title,
            body,
            head,
            base,
        } => create(cli_hostname, cli_repo, title, body, head, base).await,
        PrCommand::Close { number } => close(cli_hostname, cli_repo, number).await,
        PrCommand::Merge { number, message } => {
            merge(cli_hostname, cli_repo, number, message).await
        }
        PrCommand::Checkout { number } => checkout(cli_hostname, cli_repo, number).await,
        PrCommand::Diff { number } => diff(cli_hostname, cli_repo, number).await,
        PrCommand::Comment { number, body } => comment(cli_hostname, cli_repo, number, body).await,
    }
}

async fn list(
    hostname: &Option<String>,
    cli_repo: &Option<String>,
    _state: &str,
    json: bool,
) -> Result<()> {
    let hostname = resolve_hostname(hostname)?;
    let (owner, repo) = resolve_repo(cli_repo)?;
    let client = create_client(&hostname)?;

    let prs = client.list_pull_requests(&owner, &repo).await?;

    if json {
        println!("{}", serde_json::to_string_pretty(&prs)?);
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

    print_table(&["#", "STATE", "TITLE", "BRANCH", "AUTHOR"], &rows);
    Ok(())
}

async fn view(
    hostname: &Option<String>,
    cli_repo: &Option<String>,
    number: u64,
    show_comments: bool,
    web: bool,
) -> Result<()> {
    let hostname = resolve_hostname(hostname)?;
    let (owner, repo) = resolve_repo(cli_repo)?;
    let client = create_client(&hostname)?;

    if web {
        let url = client.web_url(&format!("/{}/{}/pull/{}", owner, repo, number));
        open::that(&url)
            .map_err(|e| crate::error::GbError::Other(format!("Failed to open browser: {}", e)))?;
        println!("Opening {} in your browser.", url);
        return Ok(());
    }

    let pr = client.get_pull_request(&owner, &repo, number).await?;

    let state = if pr.merged == Some(true) {
        "merged"
    } else {
        &pr.state
    };

    println!("{} {}", pr.title.bold(), format!("#{}", pr.number).dimmed());
    println!("{}", format_state(state));
    println!();

    if let Some(head) = &pr.head {
        if let Some(base) = &pr.base {
            println!("{} ← {}", base.ref_name.cyan(), head.ref_name.green());
        }
    }

    if let Some(user) = &pr.user {
        print!("Author: {}  ", user.login);
    }
    if let Some(created) = &pr.created_at {
        print!("Created: {}", created);
    }
    println!();

    if let Some(body) = &pr.body {
        if !body.is_empty() {
            println!("\n{}", body);
        }
    }

    if show_comments {
        let comments = client.list_pr_comments(&owner, &repo, number).await?;
        if !comments.is_empty() {
            println!("\n{}", "--- Comments ---".dimmed());
            for c in &comments {
                let author = c
                    .user
                    .as_ref()
                    .map(|u| u.login.as_str())
                    .unwrap_or("unknown");
                let date = c.created_at.as_deref().unwrap_or("");
                println!("\n{} ({})", author.bold(), date.dimmed());
                if let Some(body) = &c.body {
                    println!("{}", body);
                }
            }
        }
    }

    Ok(())
}

async fn create(
    hostname: &Option<String>,
    cli_repo: &Option<String>,
    title: Option<String>,
    body: Option<String>,
    head: Option<String>,
    base: Option<String>,
) -> Result<()> {
    let hostname = resolve_hostname(hostname)?;
    let (owner, repo) = resolve_repo(cli_repo)?;
    let client = create_client(&hostname)?;

    let head = match head {
        Some(h) => h,
        None => {
            // Try to detect current branch
            let output = std::process::Command::new("git")
                .args(["branch", "--show-current"])
                .output();
            match output {
                Ok(o) if o.status.success() => {
                    String::from_utf8_lossy(&o.stdout).trim().to_string()
                }
                _ => Input::new().with_prompt("Head branch").interact_text()?,
            }
        }
    };

    let base = match base {
        Some(b) => b,
        None => Input::new()
            .with_prompt("Base branch")
            .default("main".to_string())
            .interact_text()?,
    };

    let title = match title {
        Some(t) => t,
        None => Input::new().with_prompt("Title").interact_text()?,
    };

    let body_text = match body {
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
        head,
        base,
        body: body_text,
    };

    let pr = client
        .create_pull_request(&owner, &repo, &create_body)
        .await?;
    println!("✓ Created pull request #{}: {}", pr.number, pr.title);
    if let Some(url) = &pr.html_url {
        println!("{}", url);
    }
    Ok(())
}

async fn close(hostname: &Option<String>, cli_repo: &Option<String>, number: u64) -> Result<()> {
    let hostname = resolve_hostname(hostname)?;
    let (owner, repo) = resolve_repo(cli_repo)?;
    let client = create_client(&hostname)?;

    // GitBucket uses PATCH on issues endpoint to close PRs
    let body = crate::models::issue::UpdateIssue {
        state: Some("closed".to_string()),
        title: None,
        body: None,
    };
    client.update_issue(&owner, &repo, number, &body).await?;
    println!("✓ Closed pull request #{}", number);
    Ok(())
}

async fn merge(
    hostname: &Option<String>,
    cli_repo: &Option<String>,
    number: u64,
    message: Option<String>,
) -> Result<()> {
    let hostname = resolve_hostname(hostname)?;
    let (owner, repo) = resolve_repo(cli_repo)?;
    let client = create_client(&hostname)?;

    let body = MergePullRequest {
        commit_message: message,
        sha: None,
        merge_method: None,
    };

    let result = client
        .merge_pull_request(&owner, &repo, number, &body)
        .await?;
    if result.merged == Some(true) {
        println!("✓ Merged pull request #{}", number);
    } else {
        let msg = result
            .message
            .unwrap_or_else(|| "Unknown error".to_string());
        println!("✗ Failed to merge: {}", msg);
    }
    Ok(())
}

async fn checkout(hostname: &Option<String>, cli_repo: &Option<String>, number: u64) -> Result<()> {
    let hostname = resolve_hostname(hostname)?;
    let (owner, repo) = resolve_repo(cli_repo)?;
    let client = create_client(&hostname)?;

    let pr = client.get_pull_request(&owner, &repo, number).await?;
    let branch = pr
        .head
        .as_ref()
        .map(|h| h.ref_name.as_str())
        .ok_or_else(|| crate::error::GbError::Other("PR has no head branch".into()))?;

    // Fetch and checkout
    let fetch_status = std::process::Command::new("git")
        .args(["fetch", "origin", branch])
        .status()?;

    if !fetch_status.success() {
        return Err(crate::error::GbError::Other("git fetch failed".into()));
    }

    let checkout_status = std::process::Command::new("git")
        .args(["checkout", branch])
        .status()?;

    if !checkout_status.success() {
        return Err(crate::error::GbError::Other("git checkout failed".into()));
    }

    println!("✓ Checked out branch '{}' for PR #{}", branch, number);
    Ok(())
}

async fn diff(hostname: &Option<String>, cli_repo: &Option<String>, number: u64) -> Result<()> {
    let hostname = resolve_hostname(hostname)?;
    let (owner, repo) = resolve_repo(cli_repo)?;
    let client = create_client(&hostname)?;

    let pr = client.get_pull_request(&owner, &repo, number).await?;
    let head = pr
        .head
        .as_ref()
        .map(|h| h.ref_name.as_str())
        .unwrap_or("HEAD");
    let base = pr
        .base
        .as_ref()
        .map(|b| b.ref_name.as_str())
        .unwrap_or("main");

    // Fetch both branches and show diff
    let _ = std::process::Command::new("git")
        .args(["fetch", "origin", head, base])
        .status();

    let status = std::process::Command::new("git")
        .args(["diff", &format!("origin/{}...origin/{}", base, head)])
        .status()?;

    if !status.success() {
        return Err(crate::error::GbError::Other("git diff failed".into()));
    }

    Ok(())
}

async fn comment(
    hostname: &Option<String>,
    cli_repo: &Option<String>,
    number: u64,
    body: Option<String>,
) -> Result<()> {
    let hostname = resolve_hostname(hostname)?;
    let (owner, repo) = resolve_repo(cli_repo)?;
    let client = create_client(&hostname)?;

    let body_text = match body {
        Some(b) => b,
        None => Input::new().with_prompt("Comment body").interact_text()?,
    };

    let comment_body = CreateComment { body: body_text };
    client
        .create_pr_comment(&owner, &repo, number, &comment_body)
        .await?;
    println!("✓ Added comment to PR #{}", number);
    Ok(())
}
