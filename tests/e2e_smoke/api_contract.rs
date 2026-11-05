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
