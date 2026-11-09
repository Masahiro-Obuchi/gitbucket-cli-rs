//! Contracts of the pinned GitBucket 4.44.0 server, not desired CLI semantics.
//! Exercise REST directly so a CLI workaround cannot hide a server limitation.

use super::support::*;
use reqwest::{Client, Method, Response, StatusCode};
use serde_json::{json, Value};
use std::time::Duration;
use tempfile::tempdir;

fn client() -> Client {
    Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .unwrap()
}

#[tokio::test]
#[ignore = "requires the pinned Docker GitBucket fixture"]
async fn contract_deleted_fork_keeps_saved_shas_and_fetchable_pr_ref() {
    let temp = tempdir().unwrap();
    login(temp.path());
    let client = client();
    let name = format!("snapshot-fork-{}", unique_suffix());
    success(
        request(
            &client,
            Method::POST,
            "user/repos",
            Some(json!({"name":name,"private":false})),
        )
        .await,
    )
    .await;
    let repo = format!("{}/{name}", required_env("GB_E2E_USER"));
    let work = temp.path().join("source");
    clone_repo_to(temp.path(), &repo, &work);
    ensure_remote_main(&work);
    run_git(&work, &["checkout", "-b", "saved-topic"]);
    std::fs::write(work.join("saved.txt"), "saved fork PR\n").unwrap();
    run_git(&work, &["add", "saved.txt"]);
    run_git(&work, &["commit", "-m", "saved fork PR"]);
    run_git(&work, &["push", "origin", "saved-topic"]);
    let saved_head = git_output(&work, &["rev-parse", "HEAD"]);
    let mut fork = gb_command();
    for (key, value) in e2e_env(temp.path()) {
        fork.env(key, value);
    }
    fork.current_dir(temp.path())
        .env("GB_WEB_USER", "root")
        .env("GB_PASSWORD", "root")
        .args(["repo", "fork", &repo, "--group", "root"]);
    run_and_assert_success(&mut fork);
    let created = success(
        request(
            &client,
            Method::POST,
            &format!("repos/{repo}/pulls"),
            Some(json!({"head":"root:saved-topic","base":"main","title":"deleted fork snapshot"})),
        )
        .await,
    )
    .await;
    let created: Value = serde_json::from_str(created.as_str().unwrap()).unwrap();
    let number = created["number"].as_u64().unwrap();
    let number_arg = number.to_string();
    gb_output_with_env(
        temp.path(),
        temp.path(),
        &["pr", "close", &number_arg, "--repo", &repo],
    );
    run_git(&work, &["push", "origin", "--delete", "saved-topic"]);
    let mut delete = gb_command();
    for (key, value) in e2e_env(temp.path()) {
        delete.env(key, value);
    }
    delete
        .current_dir(temp.path())
        .env("GB_WEB_USER", "root")
        .env("GB_PASSWORD", "root")
        .args(["repo", "delete", &format!("root/{name}"), "--yes"]);
    run_and_assert_success(&mut delete);
    let missing = request(
        &client,
        Method::GET,
        &format!("repos/{repo}/pulls/{number}"),
        None,
    )
    .await;
    assert_eq!(missing.status(), StatusCode::OK);
    assert!(missing.bytes().await.unwrap().is_empty());
    let listed = stored(&client, &format!("repos/{repo}/pulls?state=all")).await;
    assert_eq!(listed[0]["head"]["sha"], saved_head);
    let local = temp.path().join("local");
    clone_repo_to(temp.path(), &repo, &local);
    ensure_remote_main(&local);
    assert_eq!(
        git_output(
            &local,
            &["ls-remote", "origin", &format!("refs/pull/{number}/head")]
        )
        .split_whitespace()
        .next(),
        Some(saved_head.as_str())
    );
    let diff = gb_output_with_env(
        temp.path(),
        &local,
        &["pr", "diff", &number_arg, "--repo", &repo, "--no-pager"],
    );
    assert!(diff.contains("+saved fork PR"), "{diff}");
    gb_output_with_env(
        temp.path(),
        &local,
        &["pr", "checkout", &number_arg, "--repo", &repo],
    );
    assert_eq!(git_output(&local, &["rev-parse", "HEAD"]), saved_head);
}

async fn request(client: &Client, method: Method, path: &str, body: Option<Value>) -> Response {
    let mut request = client
        .request(
            method,
            format!("{}/api/v3/{path}", required_env("GB_E2E_BASE_URL")),
        )
        .header(
            "Authorization",
            format!("token {}", required_env("GB_E2E_TOKEN")),
        );
    if let Some(body) = body {
        request = request.json(&body);
    }
    request.send().await.unwrap()
}

async fn success(response: Response) -> Value {
    let status = response.status();
    let text = response.text().await.unwrap();
    assert!(status.is_success(), "HTTP {status}: {text}");
    serde_json::from_str(&text).unwrap()
}

async fn stored(client: &Client, path: &str) -> Value {
    success(request(client, Method::GET, path, None).await).await
}

#[tokio::test]
#[ignore = "requires the pinned Docker GitBucket fixture"]
async fn contract_pr_create_string_and_merge_action_result_wrappers_are_route_specific() {
    let temp = tempdir().unwrap();
    login(temp.path());
    let (_, branch, _) = create_live_pr_fixture(temp.path(), "contract-response");
    // A dedicated base branch avoids racing other E2E merges into main.
    let base = format!("{branch}-base");
    let work = temp.path().join(format!("work-{branch}"));
    run_git(&work, &["branch", &base, "HEAD^"]);
    run_git(&work, &["push", "origin", &base]);
    let client = client();
    let path = format!("repos/{}/pulls", required_env("GB_E2E_REPO"));
    let title = format!("response contract {}", unique_suffix());
    let response = request(
        &client,
        Method::POST,
        &path,
        Some(json!({"title":title,"head":branch,"base":base,"body":"contract"})),
    )
    .await;
    assert!(response.status().is_success());
    let outer: Value = serde_json::from_str(&response.text().await.unwrap()).unwrap();
    let created: Value = serde_json::from_str(
        outer
            .as_str()
            .expect("4.44 create must be a JSON string wrapper"),
    )
    .unwrap();
    assert_eq!(created["title"], title);
    assert_eq!(created["head"]["ref"], branch);
    assert_eq!(created["base"]["ref"], base);
    let number = created["number"].as_u64().unwrap();
    let response = request(
        &client,
        Method::PUT,
        &format!("{path}/{number}/merge"),
        Some(json!({"commit_message":"response contract"})),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let outer: Value = serde_json::from_str(&response.text().await.unwrap()).unwrap();
    let envelope = outer
        .as_object()
        .expect("4.44 merge must serialize its ActionResult");
    assert_eq!(envelope.len(), 3);
    assert_eq!(outer["status"], 200);
    assert!(outer["headers"].is_object());
    let merged: Value = serde_json::from_str(outer["body"].as_str().unwrap()).unwrap();
    assert_eq!(merged["merged"], true);
    assert!(merged["sha"].is_string());
    assert_eq!(
        stored(&client, &format!("{path}/{number}")).await["merged"],
        true
    );
}

#[tokio::test]
#[ignore = "requires the pinned Docker GitBucket fixture"]
async fn contract_pr_patch_and_inline_comments_have_distinct_api_paths() {
    let temp = tempdir().unwrap();
    login(temp.path());
    let repo = required_env("GB_E2E_REPO");
    let (number, _, file) = create_live_pr_fixture(temp.path(), "contract-inline");
    let client = client();
    let path = format!("repos/{repo}/pulls/{number}");
    let title = format!("contract-edited-{}", unique_suffix());

    let updated = success(
        request(
            &client,
            Method::PATCH,
            &path,
            Some(json!({"title": title, "body": "REST edited body", "state": "closed"})),
        )
        .await,
    )
    .await;
    assert_eq!(updated["title"], title);
    assert_eq!(updated["body"], "REST edited body");
    assert_eq!(updated["state"], "closed");
    let persisted = stored(&client, &path).await;
    assert_eq!(persisted["title"], title);
    assert_eq!(persisted["body"], "REST edited body");
    assert_eq!(persisted["state"], "closed");
    let reopened = success(
        request(
            &client,
            Method::PATCH,
            &path,
            Some(json!({"state": "open"})),
        )
        .await,
    )
    .await;
    assert_eq!(reopened["state"], "open");
    assert_eq!(reopened["title"], title);
    assert_eq!(reopened["body"], "REST edited body");

    let issue_patch = request(
        &client,
        Method::PATCH,
        &format!("repos/{repo}/issues/{number}"),
        Some(json!({"title": "must-not-be-applied"})),
    )
    .await;
    assert_eq!(issue_patch.status(), StatusCode::NOT_FOUND);
    let pr = stored(&client, &path).await;
    assert_eq!(pr["title"], title);
    assert_eq!(pr["body"], "REST edited body");
    assert_eq!(pr["state"], "open");
    let head_sha = pr["head"]["sha"].as_str().unwrap();

    // A positive control ensures an empty/broken listing cannot prove that
    // inline comments are excluded from an otherwise working comments API.
    let comments_path = format!("repos/{repo}/issues/{number}/comments");
    let ordinary_marker = format!("ordinary-contract-{}", unique_suffix());
    let ordinary = success(
        request(
            &client,
            Method::POST,
            &comments_path,
            Some(json!({"body": ordinary_marker})),
        )
        .await,
    )
    .await;
    assert_eq!(ordinary["body"], ordinary_marker);
    let ordinary_id = ordinary["id"].as_u64().unwrap();

    let marker = format!("inline-contract-{}", unique_suffix());
    let web = Client::builder()
        .cookie_store(true)
        .timeout(Duration::from_secs(30))
        .build()
        .unwrap();
    let base = required_env("GB_E2E_BASE_URL");
    let response = web
        .post(format!("{base}/signin"))
        .form(&[
            ("userName", required_env("GB_E2E_USER")),
            ("password", required_env("GB_E2E_PASSWORD")),
            ("hash", String::new()),
        ])
        .send()
        .await
        .unwrap();
    assert!(response.status().is_success());
    assert!(!response.url().path().ends_with("/signin"));

    let response = web
        .post(format!("{base}/{repo}/commit/{head_sha}/comment/new"))
        .form(&[
            ("fileName", file.clone()),
            ("newLineNumber", "1".to_string()),
            ("content", marker.clone()),
            ("issueId", number.to_string()),
        ])
        .send()
        .await
        .unwrap();
    assert!(response.status().is_success());
    let page = web
        .get(format!("{base}/{repo}/pull/{number}"))
        .send()
        .await
        .unwrap();
    assert!(page.status().is_success());
    let html = page.text().await.unwrap();
    assert!(
        html.contains(&marker),
        "inline comment missing from PR page"
    );
    assert!(html.contains(&file), "inline file missing from PR page");
    let comments = stored(&client, &comments_path).await;
    let comments = comments.as_array().unwrap();
    assert!(
        comments
            .iter()
            .any(|comment| { comment["id"] == ordinary_id && comment["body"] == ordinary_marker }),
        "ordinary comment missing from Issue comments API"
    );
    for comment in comments {
        let body = comment["body"]
            .as_str()
            .expect("comment body must be a string");
        assert!(
            !body.contains(&marker),
            "inline comment leaked into Issue comments API"
        );
    }
    assert_eq!(
        request(&client, Method::GET, &format!("{path}/comments"), None)
            .await
            .status(),
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
#[ignore = "requires the pinned Docker GitBucket fixture"]
async fn contract_milestone_patch_requires_complete_preserved_values() {
    let client = client();
    let path = format!("repos/{}/milestones", required_env("GB_E2E_REPO"));
    let title = format!("contract-milestone-{}", unique_suffix());
    let created = success(
        request(
            &client,
            Method::POST,
            &path,
            Some(json!({"title": title, "description": "preserve-description", "due_on": "2027-04-01T00:00:00Z"})),
        )
        .await,
    )
    .await;
    let number = created["number"].as_u64().unwrap();
    let item = format!("{path}/{number}");
    let initial = stored(&client, &item).await;
    assert_eq!(initial["title"], title);
    assert_eq!(initial["state"], "open");
    assert_eq!(initial["description"], "preserve-description");
    assert_eq!(initial["due_on"], "2027-04-01T00:00:00Z");
    let state_only = request(
        &client,
        Method::PATCH,
        &item,
        Some(json!({"state": "closed"})),
    )
    .await;
    assert_eq!(state_only.status(), StatusCode::NOT_FOUND);
    let unchanged = stored(&client, &item).await;
    for field in ["title", "state", "description", "due_on"] {
        assert_eq!(
            unchanged[field], initial[field],
            "state-only PATCH changed {field}"
        );
    }
    let updated = success(
        request(
            &client,
            Method::PATCH,
            &item,
            Some(json!({"title": title, "state": "closed", "description": initial["description"], "due_on": initial["due_on"]})),
        )
        .await,
    )
    .await;
    assert_eq!(updated["state"], "closed");
    assert_eq!(updated["description"], initial["description"]);
    assert_eq!(updated["due_on"], initial["due_on"]);
    let persisted = stored(&client, &item).await;
    assert_eq!(persisted["title"], title);
    assert_eq!(persisted["state"], "closed");
    assert_eq!(persisted["description"], "preserve-description");
    assert_eq!(persisted["due_on"], "2027-04-01T00:00:00Z");

    // Explicit JSON null is the clear representation, not an empty date string.
    let cleared = success(
        request(
            &client,
            Method::PATCH,
            &item,
            Some(json!({"title": title, "state": "closed", "description": null, "due_on": null})),
        )
        .await,
    )
    .await;
    let stored_clear = stored(&client, &item).await;
    for value in [&cleared, &stored_clear] {
        assert_eq!(value["title"], title);
        assert_eq!(value["state"], "closed");
        assert_eq!(value["description"], "");
        assert!(value.get("due_on").is_none_or(Value::is_null));
    }

    // Document the server's replacement semantics; the CLI must compensate.
    let replaced =
        success(request(&client, Method::PATCH, &item, Some(json!({"title": title}))).await).await;
    assert_eq!(replaced["state"], "open");
    let persisted = stored(&client, &item).await;
    assert_eq!(persisted["title"], title);
    assert_eq!(persisted["state"], "open");
    assert_eq!(persisted["description"], "");
    assert!(persisted.get("due_on").is_none_or(Value::is_null));
    assert_eq!(replaced["description"], persisted["description"]);
    assert_eq!(replaced["due_on"], persisted["due_on"]);
    assert_eq!(
        request(
            &client,
            Method::PATCH,
            &item,
            Some(json!({"title": "日本語 milestone"})),
        )
        .await
        .status(),
        StatusCode::NOT_FOUND
    );
    let unchanged = stored(&client, &item).await;
    for field in ["title", "state", "description", "due_on"] {
        assert_eq!(
            unchanged[field], persisted[field],
            "invalid title PATCH changed {field}"
        );
    }
    let deleted = request(&client, Method::DELETE, &item, None).await;
    assert_eq!(deleted.status(), StatusCode::NO_CONTENT);
    assert_eq!(
        request(&client, Method::GET, &item, None).await.status(),
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
#[ignore = "requires the pinned Docker GitBucket fixture"]
async fn contract_label_updates_require_name_and_color_and_reject_same_name() {
    let client = client();
    let path = format!("repos/{}/labels", required_env("GB_E2E_REPO"));
    let name = format!("contract-label-{}", unique_suffix());
    let created = success(
        request(
            &client,
            Method::POST,
            &path,
            Some(json!({"name": name, "color": "123abc", "description": "not-supported"})),
        )
        .await,
    )
    .await;
    assert!(created.get("description").is_none_or(Value::is_null));
    let item = format!("{path}/{name}");
    let initial = stored(&client, &item).await;
    assert_eq!(initial["name"], name);
    assert_eq!(initial["color"], "123abc");
    assert!(initial.get("description").is_none_or(Value::is_null));
    assert_eq!(
        request(
            &client,
            Method::PATCH,
            &item,
            Some(json!({"color": "abcdef"}))
        )
        .await
        .status(),
        StatusCode::NOT_FOUND
    );
    let renamed = format!("{name}-renamed");
    assert_eq!(
        request(
            &client,
            Method::PATCH,
            &item,
            Some(json!({"name": renamed})),
        )
        .await
        .status(),
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        request(
            &client,
            Method::PATCH,
            &item,
            Some(json!({"name": name, "color": "abcdef"})),
        )
        .await
        .status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );
    let unchanged = stored(&client, &item).await;
    assert_eq!(unchanged["name"], name);
    assert_eq!(unchanged["color"], "123abc");

    // A complete PATCH to a fresh name proves the update route itself works.
    let updated = success(
        request(
            &client,
            Method::PATCH,
            &item,
            Some(json!({"name": renamed, "color": "abcdef"})),
        )
        .await,
    )
    .await;
    assert_eq!(updated["name"], renamed);
    assert_eq!(updated["color"], "abcdef");
    let renamed_item = format!("{path}/{renamed}");
    let persisted = stored(&client, &renamed_item).await;
    assert_eq!(persisted["name"], renamed);
    assert_eq!(persisted["color"], "abcdef");
    assert_eq!(
        request(&client, Method::GET, &item, None).await.status(),
        StatusCode::NOT_FOUND
    );
    let deleted = request(&client, Method::DELETE, &renamed_item, None).await;
    assert_eq!(deleted.status(), StatusCode::NO_CONTENT);
    assert_eq!(
        request(&client, Method::GET, &renamed_item, None)
            .await
            .status(),
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
#[ignore = "requires the pinned Docker GitBucket fixture"]
async fn contract_pr_content_patch_requires_a_title_body_pair() {
    let temp = tempdir().unwrap();
    login(temp.path());
    let (number, _, _) = create_live_pr_fixture(temp.path(), "contract-pr-content");
    let client = client();
    let path = format!("repos/{}/pulls/{number}", required_env("GB_E2E_REPO"));
    let original = stored(&client, &path).await;
    assert!(!original["body"].as_str().unwrap_or_default().is_empty());
    let ignored = success(
        request(
            &client,
            Method::PATCH,
            &path,
            Some(json!({"body":"body-only-is-ignored"})),
        )
        .await,
    )
    .await;
    assert_eq!(ignored["title"], original["title"]);
    assert_eq!(ignored["body"], original["body"]);
    assert_eq!(stored(&client, &path).await["body"], original["body"]);
    let cleared = success(
        request(
            &client,
            Method::PATCH,
            &path,
            Some(json!({"title":"contract-title-only"})),
        )
        .await,
    )
    .await;
    assert!(cleared["body"].is_null() || cleared["body"] == "");
    assert!(
        stored(&client, &path).await["body"].is_null()
            || stored(&client, &path).await["body"] == ""
    );
    let fixed = success(
        request(
            &client,
            Method::PATCH,
            &path,
            Some(json!({"title":original["title"],"body":"paired-body"})),
        )
        .await,
    )
    .await;
    assert_eq!(fixed["body"], "paired-body");
    assert_eq!(stored(&client, &path).await["body"], "paired-body");
}

fn collection_cli(temp: &std::path::Path, repo: &str, kind: &str, state: &str) -> Value {
    serde_json::from_str(&gb_output_with_env(
        temp,
        temp,
        &[kind, "list", "--repo", repo, "--state", state, "--json"],
    ))
    .unwrap()
}

fn numbers(value: &Value) -> Vec<u64> {
    value
        .as_array()
        .unwrap()
        .iter()
        .map(|row| row["number"].as_u64().unwrap())
        .collect()
}

async fn assert_fixed_collection(client: &Client, repo: &str, route: &str, expected: &[u64]) {
    let mut actual = Vec::new();
    for page in 1..=expected.len() / 25 + 1 {
        // 4.44.0 ignores per_page=1; it uses fixed pages of 25 and no Link.
        let response = request(
            client,
            Method::GET,
            &format!("repos/{repo}/{route}?state=all&page={page}&per_page=1"),
            None,
        )
        .await;
        assert!(!response.headers().contains_key("link"));
        let rows = success(response).await;
        assert_eq!(
            rows.as_array().unwrap().len(),
            expected.len().saturating_sub((page - 1) * 25).min(25)
        );
        actual.extend(numbers(&rows));
    }
    actual.sort_unstable();
    assert_eq!(actual, expected);
}

#[tokio::test]
#[ignore = "requires the pinned Docker GitBucket fixture"]
async fn contract_fixed_pages_all_comments_and_canonical_fork_identity() {
    let temp = tempdir().unwrap();
    login(temp.path());
    let client = client();
    let name = format!("pagination-{}", unique_suffix());
    let owner = required_env("GB_E2E_USER");
    let repo = format!("{owner}/{name}");
    let created = success(
        request(
            &client,
            Method::POST,
            "user/repos",
            Some(json!({"name":name,"auto_init":true,"private":false})),
        )
        .await,
    )
    .await;
    let base = created["default_branch"].as_str().unwrap();
    let work = temp.path().join("work");
    clone_repo_to(temp.path(), &repo, &work);
    configure_git_identity(&work);
    std::fs::write(work.join("pages.txt"), "pagination\n").unwrap();
    run_git(&work, &["add", "pages.txt"]);
    run_git(&work, &["commit", "-m", "Add pagination fixture"]);
    let refs: Vec<_> = (1..=51)
        .map(|n| format!("HEAD:refs/heads/pages-topic-{n}"))
        .collect();
    let mut push = vec!["push", "origin"];
    push.extend(refs.iter().map(String::as_str));
    run_git(&work, &push);

    let mut issues = Vec::new();
    let mut prs = Vec::new();
    for count in [0, 1, 25, 26, 50, 51] {
        while issues.len() < count {
            let n = issues.len() + 1;
            let issue = success(
                request(
                    &client,
                    Method::POST,
                    &format!("repos/{repo}/issues"),
                    Some(json!({"title":format!("Issue {n}")})),
                )
                .await,
            )
            .await;
            issues.push(issue["number"].as_u64().unwrap());
            let pr = success(request(&client, Method::POST, &format!("repos/{repo}/pulls"), Some(json!({"title":format!("PR {n}"),"head":format!("pages-topic-{n}"),"base":base}))).await).await;
            let pr: Value = serde_json::from_str(pr.as_str().unwrap()).unwrap();
            prs.push(pr["number"].as_u64().unwrap());
        }
        for (kind, route, expected) in [("issue", "issues", &issues), ("pr", "pulls", &prs)] {
            assert_fixed_collection(&client, &repo, route, expected).await;
            let mut descending = expected.clone();
            descending.reverse();
            assert_eq!(
                numbers(&collection_cli(temp.path(), &repo, kind, "all")),
                descending
            );
        }
    }
    // The oldest open PR is beyond the first two pages.
    let existing = gb_output_with_env(
        temp.path(),
        temp.path(),
        &[
            "pr",
            "create",
            "--repo",
            &repo,
            "--head",
            "pages-topic-1",
            "--base",
            base,
            "--detect-existing",
            "--json",
        ],
    );
    assert_eq!(
        serde_json::from_str::<Value>(&existing).unwrap()["number"],
        prs[0]
    );
    let number = prs[0].to_string();
    let comments_path = format!("repos/{repo}/issues/{number}/comments");
    let mut comment_ids = Vec::new();
    for count in [0, 1, 25, 26, 50, 51] {
        while comment_ids.len() < count {
            let comment = success(
                request(
                    &client,
                    Method::POST,
                    &comments_path,
                    Some(json!({"body":format!("comment {}", comment_ids.len())})),
                )
                .await,
            )
            .await;
            comment_ids.push(comment["id"].as_u64().unwrap());
        }
        let response = request(
            &client,
            Method::GET,
            &format!("{comments_path}?page=2&per_page=1"),
            None,
        )
        .await;
        assert!(!response.headers().contains_key("link"));
        assert_eq!(success(response).await.as_array().unwrap().len(), count);
        let listed = gb_output_with_env(
            temp.path(),
            temp.path(),
            &["pr", "comment", "list", &number, "--repo", &repo, "--json"],
        );
        let rows: Value = serde_json::from_str(&listed).unwrap();
        assert_eq!(
            rows.as_array()
                .unwrap()
                .iter()
                .map(|v| v["id"].as_u64().unwrap())
                .collect::<Vec<_>>(),
            comment_ids
        );
    }
    let viewed = gb_output_with_env(
        temp.path(),
        temp.path(),
        &[
            "pr",
            "view",
            &number,
            "--repo",
            &repo,
            "--comments",
            "--json",
        ],
    );
    assert_eq!(
        serde_json::from_str::<Value>(&viewed).unwrap()["comments"]
            .as_array()
            .unwrap()
            .len(),
        51
    );
    gb_output_with_env(
        temp.path(),
        temp.path(),
        &[
            "pr",
            "comment",
            &number,
            "--repo",
            &repo,
            "--edit-last",
            "--body",
            "latest edited",
        ],
    );
    assert_eq!(
        stored(
            &client,
            &format!(
                "repos/{repo}/issues/comments/{}",
                comment_ids.last().unwrap()
            )
        )
        .await["body"],
        "latest edited"
    );

    for (kind, number) in [("issue", issues[0]), ("pr", prs[0])] {
        gb_output_with_env(
            temp.path(),
            temp.path(),
            &[kind, "close", &number.to_string(), "--repo", &repo],
        );
        for (state, count) in [("open", 50), ("closed", 1), ("all", 51)] {
            assert_eq!(
                collection_cli(temp.path(), &repo, kind, state)
                    .as_array()
                    .unwrap()
                    .len(),
                count
            );
        }
    }

    // Fork as the fixture's root actor. The source is public, and all branch
    // refs already exist in the fork; no root Git credentials are needed.
    let mut command = gb_command();
    for (key, value) in e2e_env(temp.path()) {
        command.env(key, value);
    }
    command
        .current_dir(temp.path())
        .env("GB_WEB_USER", "root")
        .env("GB_PASSWORD", "root")
        .args(["repo", "fork", &repo, "--group", "root"]);
    run_and_assert_success(&mut command);
    let fork_pr = success(
        request(
            &client,
            Method::POST,
            &format!("repos/{repo}/pulls"),
            Some(json!({"title":"Fork pagination","head":"root:pages-topic-1","base":base})),
        )
        .await,
    )
    .await;
    let fork_pr: Value = serde_json::from_str(fork_pr.as_str().unwrap()).unwrap();
    let fork_number = fork_pr["number"].as_u64().unwrap();
    let expected_head = format!("root/{name}");
    assert_eq!(
        stored(&client, &format!("repos/{repo}/pulls/{fork_number}")).await["head"]["repo"]
            ["full_name"],
        expected_head
    );
    let summary = stored(&client, &format!("repos/{repo}/pulls?state=all")).await;
    let summary = summary
        .as_array()
        .unwrap()
        .iter()
        .find(|pr| pr["number"] == fork_number)
        .unwrap();
    assert_eq!(
        summary["head"]["repo"]["full_name"], repo,
        "4.44 collection misidentifies a fork as base-owned"
    );
    let listed = collection_cli(temp.path(), &repo, "pr", "all");
    assert_eq!(listed[0]["head"]["repo"]["full_name"], expected_head);
    let existing = gb_output_with_env(
        temp.path(),
        temp.path(),
        &[
            "pr",
            "create",
            "--repo",
            &repo,
            "--head",
            "pages-topic-1",
            "--head-owner",
            "root",
            "--base",
            base,
            "--detect-existing",
            "--json",
        ],
    );
    assert_eq!(
        serde_json::from_str::<Value>(&existing).unwrap()["number"],
        fork_number
    );
}

#[tokio::test]
#[ignore = "requires the pinned Docker GitBucket fixture"]
async fn contract_cli_json_preserves_api_fields_and_separates_issue_comment_details() {
    let temp = tempdir().unwrap();
    login(temp.path());
    let client = client();
    let repo = required_env("GB_E2E_REPO");
    let (_, branch, _) = create_live_pr_fixture(temp.path(), "json-fields");
    let work = temp.path().join(format!("work-{branch}"));
    let base = format!("{branch}-base");
    run_git(&work, &["branch", &base, "HEAD^"]);
    run_git(&work, &["push", "origin", &base]);
    let created = success(request(&client, Method::POST, &format!("repos/{repo}/pulls"), Some(json!({"title":"日本語 Draft 🌸","head":branch,"base":base,"body":"原本の本文","draft":true}))).await).await;
    let created: Value = serde_json::from_str(created.as_str().unwrap()).unwrap();
    let pr = created["number"].as_u64().unwrap();
    add_pr_comment(temp.path(), pr, "PRの通常コメント");
    let issue = create_live_issue(temp.path(), "JSON Issue 🌸", "原本のIssue本文");
    add_issue_comment(temp.path(), issue, "Issueの通常コメント");

    for (kind, route, number) in [("issue", "issues", issue), ("pr", "pulls", pr)] {
        let number_arg = number.to_string();
        let api = stored(&client, &format!("repos/{repo}/{route}/{number}")).await;
        let plain = gb_output_with_env(
            temp.path(),
            temp.path(),
            &[kind, "view", &number_arg, "--json"],
        );
        assert_eq!(serde_json::from_str::<Value>(&plain).unwrap(), api);
        // These fields were previously dropped by typed serialization.
        assert!(api.get("id").is_some());
        assert!(api.get("comments_url").is_some());
        let mut comments = stored(&client, &format!("repos/{repo}/issues/{number}/comments")).await;
        comments
            .as_array_mut()
            .unwrap()
            .sort_by_key(|row| row["id"].as_u64().unwrap());
        let combined = gb_output_with_env(
            temp.path(),
            temp.path(),
            &[kind, "view", &number_arg, "--comments", "--json"],
        );
        let mut expected = api.clone();
        if kind == "issue" {
            expected["comment_details"] = comments;
            // 4.44.0 omits the count; the CLI must not invent one.
            assert!(api.get("comments").is_none());
        } else {
            assert_eq!(api["draft"], true);
            assert!(api["labels"].is_array() && api["assignees"].is_array());
            assert!(api["mergeable"].is_null());
            if let Some(original) = api.get("comments") {
                expected["api_comments"] = original.clone();
            }
            expected["comments"] = comments.clone();
            let listed = gb_output_with_env(
                temp.path(),
                temp.path(),
                &["pr", "comment", "list", &number_arg, "--json"],
            );
            assert_eq!(serde_json::from_str::<Value>(&listed).unwrap(), comments);
            let view = gb_output_with_env(
                temp.path(),
                temp.path(),
                &["pr", "view", &number_arg, "--no-pager"],
            );
            for field in ["DRAFT", "Draft: yes", "Labels: (none)", "Assignees: (none)"] {
                assert!(view.contains(field), "{view}");
            }
        }
        assert_eq!(serde_json::from_str::<Value>(&combined).unwrap(), expected);
    }
}

#[tokio::test]
#[ignore = "requires the pinned Docker GitBucket fixture"]
async fn contract_review_comment_markup_and_visibility() {
    use scraper::{Html, Selector};
    let temp = tempdir().unwrap();
    login(temp.path());
    let repo = required_env("GB_E2E_REPO");
    let client = client();
    let suffix = unique_suffix();
    let branch = format!("review-topic-{suffix}");
    let base_branch = format!("review-base-{suffix}");
    let work = temp.path().join("work");
    clone_repo_to(temp.path(), &repo, &work);
    ensure_remote_main(&work);
    run_git(&work, &["checkout", "-b", &base_branch]);
    std::fs::write(work.join("README.md"), "alpha\nbeta\ngamma\ndelta\n").unwrap();
    std::fs::write(work.join("gone.txt"), "deleted file\n").unwrap();
    run_git(&work, &["add", "."]);
    run_git(&work, &["commit", "-m", "review base files"]);
    run_git(&work, &["push", "origin", &base_branch]);
    run_git(&work, &["checkout", "-b", &branch]);
    run_git(&work, &["mv", "README.md", "renamed file.txt"]);
    std::fs::write(
        work.join("renamed file.txt"),
        "alpha\nbeta\ngamma\ndelta\nnew\n",
    )
    .unwrap();
    std::fs::write(work.join("new file.txt"), "new first\nnew second\n").unwrap();
    std::fs::write(work.join(" review whitespace.txt "), "whitespace path\n").unwrap();
    run_git(&work, &["rm", "gone.txt"]);
    run_git(&work, &["add", "."]);
    run_git(&work, &["commit", "-m", "rename delete and add"]);
    run_git(&work, &["push", "origin", &branch]);
    let number = parse_pr_number(&gb_output_with_env(
        temp.path(),
        &work,
        &[
            "pr",
            "create",
            "--head",
            &branch,
            "--base",
            &base_branch,
            "--title",
            "Review collection",
            "--body",
            "real rename/deleted-file fixture",
        ],
    ));
    let number_arg = number.to_string();
    add_pr_comment(temp.path(), number, "ordinary control");
    let pr = stored(&client, &format!("repos/{repo}/pulls/{number}")).await;
    let sha = pr["head"]["sha"].as_str().unwrap();
    let base = required_env("GB_E2E_BASE_URL");
    let web = Client::builder()
        .cookie_store(true)
        .timeout(Duration::from_secs(30))
        .build()
        .unwrap();
    assert!(web
        .post(format!("{base}/signin"))
        .form(&[
            ("userName", required_env("GB_E2E_USER")),
            ("password", required_env("GB_E2E_PASSWORD")),
            ("hash", String::new())
        ])
        .send()
        .await
        .unwrap()
        .status()
        .is_success());
    for (side, path, line, marker) in [
        ("newLineNumber", "new file.txt", "1", "新行 **review**"),
        ("newLineNumber", "new file.txt", "1", "同じ位置への追加"),
        (
            "newLineNumber",
            " review whitespace.txt ",
            "1",
            "空白付きファイル名の指摘",
        ),
        ("oldLineNumber", "renamed file.txt", "1", "旧行の指摘"),
        ("newLineNumber", "renamed file.txt", "5", "renameの新行"),
        ("oldLineNumber", "gone.txt", "1", "削除ファイルの指摘"),
    ] {
        assert!(web
            .post(format!("{base}/{repo}/commit/{sha}/comment/new"))
            .form(&[
                ("fileName", path.to_owned()),
                (side, line.into()),
                ("issueId", number.to_string()),
                ("content", marker.into())
            ])
            .send()
            .await
            .unwrap()
            .status()
            .is_success());
    }
    // Exercise multiple whole-commit IDs independently of the IssueComment
    // numeric-ID namespace; discussion data is read separately over REST.
    for marker in ["コミット全体 A", "コミット全体 B"] {
        assert!(web
            .post(format!("{base}/{repo}/commit/{sha}/comment/new"))
            .form(&[("issueId", number.to_string()), ("content", marker.into())])
            .send()
            .await
            .unwrap()
            .status()
            .is_success());
    }
    let root = Client::builder()
        .cookie_store(true)
        .timeout(Duration::from_secs(30))
        .build()
        .unwrap();
    assert!(root
        .post(format!("{base}/signin"))
        .form(&[("userName", "root"), ("password", "root"), ("hash", "")])
        .send()
        .await
        .unwrap()
        .status()
        .is_success());
    assert!(root
        .post(format!("{base}/{repo}/commit/{sha}/comment/new"))
        .form(&[
            ("fileName", "new file.txt".into()),
            ("newLineNumber", "2".into()),
            ("issueId", number.to_string()),
            ("content", "別作者の指摘".into())
        ])
        .send()
        .await
        .unwrap()
        .status()
        .is_success());
    let page = web
        .get(format!("{base}/{repo}/pull/{number}"))
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    let document = Html::parse_document(&page);
    let visible = document
        .select(&Selector::parse("#comment-list .commit-comment-box").unwrap())
        .count();
    assert!(visible >= 7);
    let mut public_read = gb_command();
    for (key, value) in e2e_env(temp.path()) {
        public_read.env(key, value);
    }
    public_read
        .current_dir(temp.path())
        .env_remove("GB_PASSWORD")
        .env_remove("GB_USER")
        .args([
            "pr",
            "comment",
            "list",
            &number_arg,
            "--kind",
            "all",
            "--json",
        ]);
    let list = run_and_assert_success(&mut public_read);
    let listed: Value = serde_json::from_str(&list).unwrap();
    assert_eq!(listed["comments"].as_array().unwrap().len(), 1);
    assert_eq!(
        listed["comment_coverage"]["review_access"],
        "token_or_public"
    );
    assert!(listed["comment_coverage"]["web_actor"].is_null());
    let rows = listed["review_comments"].as_array().unwrap();
    assert_eq!(rows.len(), visible);
    assert!(rows.iter().any(|r| r["author"] == "root"));
    assert!(rows
        .iter()
        .any(|r| r["position"]["path"] == "gone.txt" && r["position"]["old_line"] == 1));
    assert!(rows
        .iter()
        .any(|r| r["position"]["path"] == " review whitespace.txt "
            && r["position"]["new_line"] == 1));
    assert!(rows
        .iter()
        .any(|r| r["position"]["path"] == "renamed file.txt" && r["position"]["new_line"] == 5));
    assert!(rows.iter().any(|r| r["identity"]["kind"] == "commit"));
    for row in rows {
        assert!(row["body"]["markdown"].is_null() && row["pull_request"].is_null());
        assert_eq!(row["visible_on_pull_request"], number);
    }
    let viewed = gb_output_with_env(
        temp.path(),
        temp.path(),
        &[
            "pr",
            "view",
            &number_arg,
            "--comments",
            "--kind",
            "all",
            "--json",
        ],
    );
    let viewed: Value = serde_json::from_str(&viewed).unwrap();
    assert_eq!(viewed["comments"], listed["comments"]);
    assert_eq!(viewed["review_comments"], listed["review_comments"]);
    let only = gb_output_with_env(
        temp.path(),
        temp.path(),
        &[
            "pr",
            "comment",
            "list",
            &number_arg,
            "--kind",
            "review",
            "--json",
        ],
    );
    let only: Value = serde_json::from_str(&only).unwrap();
    assert_eq!(only["comments"], json!([]));
    assert_eq!(only["review_comments"], listed["review_comments"]);
    gb_output_with_env(
        temp.path(),
        temp.path(),
        &[
            "pr",
            "comment",
            &number_arg,
            "--edit-last",
            "--body",
            "edited ordinary only",
        ],
    );
    assert_eq!(
        stored(&client, &format!("repos/{repo}/issues/{number}/comments")).await[0]["body"],
        "edited ordinary only"
    );
    if let Ok(dir) = std::env::var("GB_REVIEW_CAPTURE_DIR") {
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            std::path::Path::new(&dir).join("real-conversation.html"),
            &page,
        )
        .unwrap();
    }
    std::fs::write(work.join("new file.txt"), "updated unrelated contents\n").unwrap();
    run_git(&work, &["rm", "renamed file.txt"]);
    run_git(&work, &["add", "."]);
    run_git(&work, &["commit", "-m", "update review positions"]);
    run_git(&work, &["push", "origin", &branch]);
    let after = gb_output_with_env(
        temp.path(),
        temp.path(),
        &[
            "pr",
            "comment",
            "list",
            &number_arg,
            "--kind",
            "review",
            "--json",
        ],
    );
    let after: Value = serde_json::from_str(&after).unwrap();
    assert!(after["review_comments"]
        .as_array()
        .unwrap()
        .iter()
        .any(|r| r["position"]["outdated"] == true));
    // Excluded commits illustrate why a successful Web-scope read cannot claim
    // to enumerate every stored historical review.
    run_git(&work, &["reset", "--hard", &base_branch]);
    std::fs::write(work.join("force.txt"), "new history\n").unwrap();
    run_git(&work, &["add", "."]);
    run_git(&work, &["commit", "-m", "replace PR history"]);
    run_git(&work, &["push", "--force", "origin", &branch]);
    let hidden = gb_output_with_env(
        temp.path(),
        temp.path(),
        &[
            "pr",
            "comment",
            "list",
            &number_arg,
            "--kind",
            "all",
            "--json",
        ],
    );
    let hidden: Value = serde_json::from_str(&hidden).unwrap();
    assert!(
        hidden["review_comments"].as_array().unwrap().len() < rows.len(),
        "The server should omit at least some reviews from the replaced history"
    );
    assert!(hidden["comment_coverage"]["historical_review_complete"].is_null());
}

#[tokio::test]
#[ignore = "requires the pinned Docker GitBucket fixture"]
async fn contract_private_review_reads_require_cookie_session_after_token_denial() {
    let temp = tempdir().unwrap();
    login(temp.path());
    let client = client();
    let name = format!("private-review-{}", unique_suffix());
    let repo = format!("{}/{name}", required_env("GB_E2E_USER"));
    success(
        request(
            &client,
            Method::POST,
            "user/repos",
            Some(json!({"name":name,"private":true})),
        )
        .await,
    )
    .await;
    let work = temp.path().join("work");
    clone_repo_to(temp.path(), &repo, &work);
    ensure_remote_main(&work);
    run_git(&work, &["checkout", "-b", "review-head"]);
    std::fs::write(work.join("README.md"), "private review change\n").unwrap();
    run_git(&work, &["add", "README.md"]);
    run_git(&work, &["commit", "-m", "private review head"]);
    run_git(&work, &["push", "origin", "review-head"]);
    let created = gb_output_with_env(
        temp.path(),
        &work,
        &[
            "pr",
            "create",
            "--repo",
            &repo,
            "--head",
            "review-head",
            "--base",
            "main",
            "--title",
            "Private review",
            "--body",
            "cookie fallback",
        ],
    );
    let number = parse_pr_number(&created);
    let number_arg = number.to_string();
    let sha = git_output(&work, &["rev-parse", "HEAD"]);
    let base = required_env("GB_E2E_BASE_URL");
    let denied = client
        .get(format!("{base}/{repo}/pull/{number}"))
        .header(
            "Authorization",
            format!("token {}", required_env("GB_E2E_TOKEN")),
        )
        .send()
        .await
        .unwrap();
    assert_eq!(denied.status(), StatusCode::UNAUTHORIZED);
    let web = Client::builder()
        .cookie_store(true)
        .timeout(Duration::from_secs(30))
        .build()
        .unwrap();
    assert!(web
        .post(format!("{base}/signin"))
        .form(&[
            ("userName", required_env("GB_E2E_USER")),
            ("password", required_env("GB_E2E_PASSWORD")),
            ("hash", String::new())
        ])
        .send()
        .await
        .unwrap()
        .status()
        .is_success());
    assert!(web
        .post(format!("{base}/{repo}/commit/{sha}/comment/new"))
        .form(&[
            ("issueId", number_arg.clone()),
            ("fileName", "README.md".into()),
            ("newLineNumber", "1".into()),
            ("content", "private inline review".into())
        ])
        .send()
        .await
        .unwrap()
        .status()
        .is_success());
    let output = gb_output_with_env(
        temp.path(),
        temp.path(),
        &[
            "pr",
            "comment",
            "list",
            &number_arg,
            "--repo",
            &repo,
            "--kind",
            "all",
            "--json",
        ],
    );
    let output: Value = serde_json::from_str(&output).unwrap();
    assert_eq!(output["review_comments"].as_array().unwrap().len(), 1);
    assert_eq!(
        output["review_comments"][0]["body"]["text"],
        "private inline review"
    );
    assert_eq!(
        output["comment_coverage"]["review_access"],
        "cookie_session"
    );
    assert_eq!(
        output["comment_coverage"]["web_actor"],
        required_env("GB_E2E_USER")
    );
}
