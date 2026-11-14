//! Pinned core contracts: decision endpoints are absent and merge sha is ignored.
use super::support::*;
use reqwest::{Client, Method, StatusCode};
use serde_json::{json, Value};

fn client() -> Client {
    Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .build()
        .unwrap()
}
async fn request(
    client: &Client,
    method: Method,
    path: &str,
    body: Option<Value>,
) -> reqwest::Response {
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
async fn stored(client: &Client, path: &str) -> Value {
    let response = request(client, Method::GET, path, None).await;
    assert_eq!(response.status(), StatusCode::OK);
    response.json().await.unwrap()
}
fn cli(temp: &std::path::Path, args: &[&str]) -> std::process::Command {
    let mut command = gb_command();
    command.current_dir(temp).args(args);
    for (key, value) in e2e_env(temp) {
        command.env(key, value);
    }
    command
}
async fn fixture(temp: &std::path::Path, client: &Client, prefix: &str, draft: bool) -> String {
    let (_, branch, _) = create_live_pr_fixture(temp, prefix);
    let work = temp.join(format!("work-{branch}"));
    let base = format!("{branch}-isolated-base");
    run_git(&work, &["branch", &base, "HEAD^"]);
    run_git(&work, &["push", "origin", &base]);
    let path = format!("repos/{}/pulls", required_env("GB_E2E_REPO"));
    let response = request(
        client,
        Method::POST,
        &path,
        Some(json!({"title":prefix,"head":branch,"base":base,"draft":draft})),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let outer: Value = response.json().await.unwrap();
    let created: Value = serde_json::from_str(outer.as_str().unwrap()).unwrap();
    format!("{path}/{}", created["number"].as_u64().unwrap())
}

#[tokio::test]
#[ignore = "requires the pinned Docker GitBucket fixture"]
async fn contract_review_decisions_are_not_core_comment_operations() {
    let temp = tempfile::tempdir().unwrap();
    login(temp.path());
    let client = client();
    let path = fixture(temp.path(), &client, "review-decisions", false).await;
    let pr = stored(&client, &path).await;
    let number = pr["number"].as_u64().unwrap().to_string();
    assert_eq!(pr["state"], "open");
    let comments = format!(
        "repos/{}/issues/{number}/comments",
        required_env("GB_E2E_REPO")
    );
    let before = stored(&client, &comments).await;
    for (method, suffix, body) in [
        (Method::GET, "/reviews", None),
        (
            Method::POST,
            "/reviews",
            Some(json!({"event":"APPROVE","body":"not a comment"})),
        ),
        (
            Method::POST,
            "/reviews",
            Some(json!({"event":"REQUEST_CHANGES","body":"not a comment"})),
        ),
        (
            Method::PUT,
            "/reviews/1/dismissals",
            Some(json!({"message":"not a comment"})),
        ),
    ] {
        let response = request(&client, method, &format!("{path}{suffix}"), body).await;
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }
    assert_eq!(stored(&client, &comments).await, before);
    let view = run_and_assert_success(&mut cli(temp.path(), &["pr", "view", &number]));
    assert!(view.contains("Review decision: unknown"));
    assert!(view.contains(pr["head"]["sha"].as_str().unwrap()));
}

#[tokio::test]
#[ignore = "requires the pinned Docker GitBucket fixture"]
async fn contract_merge_ignores_wrong_sha_while_cli_refuses_it_and_checks_readback() {
    let temp = tempfile::tempdir().unwrap();
    login(temp.path());
    let client = client();
    let path = fixture(temp.path(), &client, "merge-head-contract", false).await;
    let before = stored(&client, &path).await;
    let number = before["number"].as_u64().unwrap().to_string();
    let wrong = "1111111111111111111111111111111111111111";
    let error = run_and_assert_failure(&mut cli(
        temp.path(),
        &[
            "--json-errors",
            "pr",
            "merge",
            &number,
            "--match-head-commit",
            wrong,
        ],
    ));
    let error: Value = serde_json::from_str(&error).unwrap();
    assert_eq!(error["error"]["cause"], "head_mismatch");
    assert_eq!(stored(&client, &path).await["merged"], false);
    // This direct request deliberately demonstrates the missing server guard.
    let response = request(
        &client,
        Method::PUT,
        &format!("{path}/merge"),
        Some(json!({"sha":wrong,"commit_message":"Prove ignored sha"})),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let outer: Value = response.json().await.unwrap();
    let result: Value = serde_json::from_str(outer["body"].as_str().unwrap()).unwrap();
    // The pinned result uses JGit toString rather than a bare hexadecimal hash.
    assert!(result["sha"].as_str().unwrap().starts_with("AnyObjectId["));
    assert_eq!(result["merged"], true);
    assert_eq!(stored(&client, &path).await["merged"], true);
    let error = run_and_assert_failure(&mut cli(
        temp.path(),
        &["--json-errors", "pr", "merge", &number],
    ));
    assert!(error.contains("not_open"));

    let path = fixture(temp.path(), &client, "merge-readback", false).await;
    let before = stored(&client, &path).await;
    let number = before["number"].as_u64().unwrap().to_string();
    let sha = before["head"]["sha"].as_str().unwrap();
    let report: Value = serde_json::from_str(&run_and_assert_success(&mut cli(
        temp.path(),
        &["pr", "merge", &number, "--match-head-commit", sha, "--json"],
    )))
    .unwrap();
    assert_eq!(report["head_commit"], sha);
    assert_eq!(report["atomic"], false);
    assert_eq!(report["result"]["merged"], true);
    let after = stored(&client, &path).await;
    assert_eq!(after["state"], "closed");
    assert_eq!(after["head"]["sha"], sha);
}

#[tokio::test]
#[ignore = "requires the pinned Docker GitBucket fixture"]
async fn contract_draft_merge_is_refused_by_cli_and_server() {
    let temp = tempfile::tempdir().unwrap();
    login(temp.path());
    let client = client();
    let path = fixture(temp.path(), &client, "merge-draft", true).await;
    let before = stored(&client, &path).await;
    let number = before["number"].as_u64().unwrap().to_string();
    assert_eq!(before["draft"], true);
    let error = run_and_assert_failure(&mut cli(
        temp.path(),
        &["--json-errors", "pr", "merge", &number],
    ));
    assert!(error.contains("\"cause\":\"draft\""));
    let response = request(
        &client,
        Method::PUT,
        &format!("{path}/merge"),
        Some(json!({"commit_message":"must not merge draft"})),
    )
    .await;
    // The pinned ActionResult wrapper can carry the failure status in HTTP 200.
    let status = response.status();
    let value: Value = response.json().await.unwrap();
    assert!(status == StatusCode::METHOD_NOT_ALLOWED || value["status"] == 405);
    let after = stored(&client, &path).await;
    assert_eq!(after["merged"], false);
    assert_eq!(after["draft"], true);
    assert_eq!(after["state"], "open");
}
