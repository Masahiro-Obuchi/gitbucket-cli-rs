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
    assert_eq!(updated["state"], "closed");
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
    let pr = success(request(&client, Method::GET, &path, None).await).await;
    assert_eq!(pr["title"], title);
    let head_sha = pr["head"]["sha"].as_str().unwrap();

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
    let html = web
        .get(format!("{base}/{repo}/pull/{number}"))
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    assert!(
        html.contains(&marker),
        "inline comment missing from PR page"
    );
    assert!(html.contains(&file), "inline file missing from PR page");
    let comments = success(
        request(
            &client,
            Method::GET,
            &format!("repos/{repo}/issues/{number}/comments"),
            None,
        )
        .await,
    )
    .await;
    assert!(comments.is_array());
    assert!(comments.as_array().unwrap().iter().all(|comment| {
        comment["body"]
            .as_str()
            .is_none_or(|body| !body.contains(&marker))
    }));
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
    let state_only = request(
        &client,
        Method::PATCH,
        &item,
        Some(json!({"state": "closed"})),
    )
    .await;
    assert_eq!(state_only.status(), StatusCode::NOT_FOUND);
    let updated = success(
        request(
            &client,
            Method::PATCH,
            &item,
            Some(json!({"title": title, "state": "closed", "description": created["description"], "due_on": created["due_on"]})),
        )
        .await,
    )
    .await;
    assert_eq!(updated["state"], "closed");
    assert_eq!(updated["description"], created["description"]);
    assert_eq!(updated["due_on"], created["due_on"]);

    // Document the server's replacement semantics; the CLI must compensate.
    let replaced =
        success(request(&client, Method::PATCH, &item, Some(json!({"title": title}))).await).await;
    assert_eq!(replaced["state"], "open");
    assert_ne!(replaced["description"], created["description"]);
    assert_ne!(replaced["due_on"], created["due_on"]);
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
    let deleted = request(&client, Method::DELETE, &item, None).await;
    assert_eq!(deleted.status(), StatusCode::NO_CONTENT);
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
    let unchanged = success(request(&client, Method::GET, &item, None).await).await;
    assert_eq!(unchanged["color"], "123abc");
    let deleted = request(&client, Method::DELETE, &item, None).await;
    assert_eq!(deleted.status(), StatusCode::NO_CONTENT);
}
