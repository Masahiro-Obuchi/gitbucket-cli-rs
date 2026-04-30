mod support;

use serde_json::Value;
use support::gb_cmd::GbTestEnv;
use support::mock_http::{spawn_scripted_server, spawn_server, ScriptedResponse};

#[test]
fn issue_create_sends_labels_assignees_and_body() {
    let env = GbTestEnv::new();
    let (port, server) = spawn_server(
        "200 OK",
        r#"{"number":7,"title":"Bug report","state":"open","labels":[],"assignees":[]}"#,
    );

    let output = env
        .repo_api_command(format!("127.0.0.1:{port}"), "alice/project")
        .args([
            "issue",
            "create",
            "-t",
            "Bug report",
            "-b",
            "Body text",
            "-l",
            "bug,urgent",
            "-a",
            "alice,bob",
        ])
        .output()
        .unwrap();

    let request = server.join().unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(request.method, "POST");
    assert_eq!(request.target, "/api/v3/repos/alice/project/issues");
    let body: Value = serde_json::from_str(&request.body).unwrap();
    assert_eq!(body["title"], "Bug report");
    assert_eq!(body["body"], "Body text");
    assert_eq!(body["labels"], serde_json::json!(["bug", "urgent"]));
    assert_eq!(body["assignees"], serde_json::json!(["alice", "bob"]));
}

#[test]
fn issue_create_supports_repeated_label_and_assignee_flags() {
    let env = GbTestEnv::new();
    let (port, server) = spawn_server(
        "200 OK",
        r#"{"number":8,"title":"Another bug","state":"open","labels":[],"assignees":[]}"#,
    );

    let output = env
        .repo_api_command(format!("127.0.0.1:{port}"), "alice/project")
        .args([
            "issue",
            "create",
            "-t",
            "Another bug",
            "-b",
            "Some body",
            "-l",
            "bug",
            "-l",
            "urgent",
            "-a",
            "alice",
            "-a",
            "bob",
        ])
        .output()
        .unwrap();

    let request = server.join().unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(request.method, "POST");
    assert_eq!(request.target, "/api/v3/repos/alice/project/issues");
    let body: Value = serde_json::from_str(&request.body).unwrap();
    assert_eq!(body["title"], "Another bug");
    assert_eq!(body["labels"], serde_json::json!(["bug", "urgent"]));
    assert_eq!(body["assignees"], serde_json::json!(["alice", "bob"]));
}

#[test]
fn issue_close_sends_closed_state_patch() {
    let env = GbTestEnv::new();
    let (port, server) = spawn_server(
        "200 OK",
        r#"{"number":7,"title":"Bug report","state":"closed","labels":[]}"#,
    );

    let output = env
        .repo_api_command(format!("127.0.0.1:{port}"), "alice/project")
        .args(["issue", "close", "7"])
        .output()
        .unwrap();

    let request = server.join().unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(request.method, "PATCH");
    assert_eq!(request.target, "/api/v3/repos/alice/project/issues/7");
    let body: Value = serde_json::from_str(&request.body).unwrap();
    assert_eq!(body["state"], "closed");
    assert!(body["title"].is_null());
    assert!(body["body"].is_null());
}

#[test]
fn issue_reopen_sends_open_state_patch() {
    let env = GbTestEnv::new();
    let (port, server) = spawn_server(
        "200 OK",
        r#"{"number":7,"title":"Bug report","state":"open","labels":[]}"#,
    );

    let output = env
        .repo_api_command(format!("127.0.0.1:{port}"), "alice/project")
        .args(["issue", "reopen", "7"])
        .output()
        .unwrap();

    let request = server.join().unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(request.method, "PATCH");
    assert_eq!(request.target, "/api/v3/repos/alice/project/issues/7");
    let body: Value = serde_json::from_str(&request.body).unwrap();
    assert_eq!(body["state"], "open");
    assert!(body["title"].is_null());
    assert!(body["body"].is_null());
}

#[test]
fn issue_comment_sends_expected_payload() {
    let env = GbTestEnv::new();
    let (port, server) = spawn_server("200 OK", r#"{"id":11,"body":"Looks good"}"#);

    let output = env
        .repo_api_command(format!("127.0.0.1:{port}"), "alice/project")
        .args(["issue", "comment", "7", "--body", "Looks good"])
        .output()
        .unwrap();

    let request = server.join().unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(request.method, "POST");
    assert_eq!(
        request.target,
        "/api/v3/repos/alice/project/issues/7/comments"
    );
    let body: Value = serde_json::from_str(&request.body).unwrap();
    assert_eq!(body["body"], "Looks good");
}

#[test]
fn issue_comment_edit_last_updates_authenticated_users_latest_comment() {
    let env = GbTestEnv::new();
    let (port, server) = spawn_scripted_server(vec![
        ScriptedResponse::json(
            "GET /api/v3/user HTTP/1.1",
            "200 OK",
            r#"{"login":"alice"}"#,
        ),
        ScriptedResponse::json(
            "GET /api/v3/repos/alice/project/issues/7/comments?per_page=100 HTTP/1.1",
            "200 OK",
            r#"[
                {"id":10,"body":"Older","user":{"login":"alice"}},
                {"id":11,"body":"Other user","user":{"login":"bob"}},
                {"id":12,"body":"Latest","user":{"login":"alice"}}
            ]"#,
        ),
        ScriptedResponse::json(
            "PATCH /api/v3/repos/alice/project/issues/comments/12 HTTP/1.1",
            "200 OK",
            r#"{"id":12,"body":"Edited"}"#,
        ),
    ]);

    let output = env
        .repo_api_command(format!("127.0.0.1:{port}"), "alice/project")
        .args(["issue", "comment", "7", "--edit-last", "--body", "Edited"])
        .output()
        .unwrap();

    let requests = server.join().unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(requests.len(), 3);
    assert_eq!(requests[0].method, "GET");
    assert_eq!(requests[0].target, "/api/v3/user");
    assert_eq!(requests[1].method, "GET");
    assert_eq!(
        requests[1].target,
        "/api/v3/repos/alice/project/issues/7/comments?per_page=100"
    );
    assert_eq!(requests[2].method, "PATCH");
    assert_eq!(
        requests[2].target,
        "/api/v3/repos/alice/project/issues/comments/12"
    );
    let body: Value = serde_json::from_str(&requests[2].body).unwrap();
    assert_eq!(body["body"], "Edited");
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("Edited comment 12 on issue #7"),
        "stdout: {}",
        String::from_utf8_lossy(&output.stdout)
    );
}

#[test]
fn issue_comment_edit_last_checks_paginated_comments_before_updating() {
    let env = GbTestEnv::new();
    let (port, server) = spawn_scripted_server(vec![
        ScriptedResponse::json(
            "GET /api/v3/user HTTP/1.1",
            "200 OK",
            r#"{"login":"alice"}"#,
        ),
        ScriptedResponse::json(
            "GET /api/v3/repos/alice/project/issues/7/comments?per_page=100 HTTP/1.1",
            "200 OK",
            r#"[{"id":10,"body":"Older","user":{"login":"alice"}}]"#,
        )
        .with_header(
            "link",
            r#"</api/v3/repos/alice/project/issues/7/comments?page=2&per_page=100>; rel="next""#,
        ),
        ScriptedResponse::json(
            "GET /api/v3/repos/alice/project/issues/7/comments?page=2&per_page=100 HTTP/1.1",
            "200 OK",
            r#"[{"id":12,"body":"Latest","user":{"login":"alice"}}]"#,
        ),
        ScriptedResponse::json(
            "PATCH /api/v3/repos/alice/project/issues/comments/12 HTTP/1.1",
            "200 OK",
            r#"{"id":12,"body":"Edited"}"#,
        ),
    ]);

    let output = env
        .repo_api_command(format!("127.0.0.1:{port}"), "alice/project")
        .args(["issue", "comment", "7", "--edit-last", "--body", "Edited"])
        .output()
        .unwrap();

    let requests = server.join().unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(requests.len(), 4);
    assert_eq!(requests[0].method, "GET");
    assert_eq!(requests[0].target, "/api/v3/user");
    assert_eq!(requests[1].method, "GET");
    assert_eq!(
        requests[1].target,
        "/api/v3/repos/alice/project/issues/7/comments?per_page=100"
    );
    assert_eq!(requests[2].method, "GET");
    assert_eq!(
        requests[2].target,
        "/api/v3/repos/alice/project/issues/7/comments?page=2&per_page=100"
    );
    assert_eq!(requests[3].method, "PATCH");
    assert_eq!(
        requests[3].target,
        "/api/v3/repos/alice/project/issues/comments/12"
    );
    let body: Value = serde_json::from_str(&requests[3].body).unwrap();
    assert_eq!(body["body"], "Edited");
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("Edited comment 12 on issue #7"),
        "stdout: {}",
        String::from_utf8_lossy(&output.stdout)
    );
}

#[test]
fn issue_comment_edit_last_rejects_external_pagination_link() {
    let env = GbTestEnv::new();
    let (port, server) = spawn_scripted_server(vec![
        ScriptedResponse::json(
            "GET /api/v3/user HTTP/1.1",
            "200 OK",
            r#"{"login":"alice"}"#,
        ),
        ScriptedResponse::json(
            "GET /api/v3/repos/alice/project/issues/7/comments?per_page=100 HTTP/1.1",
            "200 OK",
            r#"[{"id":10,"body":"Older","user":{"login":"alice"}}]"#,
        )
        .with_header(
            "link",
            r#"<http://attacker.example/api/v3/repos/alice/project/issues/7/comments?page=2&per_page=100>; rel="next""#,
        ),
    ]);

    let output = env
        .repo_api_command(format!("127.0.0.1:{port}"), "alice/project")
        .args(["issue", "comment", "7", "--edit-last", "--body", "Edited"])
        .output()
        .unwrap();

    let requests = server.join().unwrap();

    assert!(!output.status.success());
    assert_eq!(requests.len(), 2);
    assert_eq!(requests[0].method, "GET");
    assert_eq!(requests[0].target, "/api/v3/user");
    assert_eq!(requests[1].method, "GET");
    assert_eq!(
        requests[1].target,
        "/api/v3/repos/alice/project/issues/7/comments?per_page=100"
    );
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .contains("Refusing to follow pagination URL outside configured GitBucket API base"),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn issue_close_falls_back_to_gitbucket_web_session() {
    let env = GbTestEnv::new();
    let (port, server) = spawn_scripted_server(vec![
        ScriptedResponse::json(
            "PATCH /gitbucket/api/v3/repos/alice/project/issues/7 HTTP/1.1",
            "404 Not Found",
            r#"{"message":"Not Found"}"#,
        ),
        ScriptedResponse::html("POST /gitbucket/signin HTTP/1.1", "200 OK", "signed in")
            .with_header("set-cookie", "JSESSIONID=session123; Path=/; HttpOnly"),
        ScriptedResponse::html(
            "POST /gitbucket/alice/project/issue_comments/state HTTP/1.1",
            "200 OK",
            "updated",
        ),
    ]);

    let output = env
        .repo_api_command(format!("127.0.0.1:{port}/gitbucket"), "alice/project")
        .env("GB_USER", "alice")
        .env("GB_PASSWORD", "secret-pass")
        .args(["issue", "close", "7"])
        .output()
        .unwrap();

    let requests = server.join().unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(requests.len(), 3);
    assert_eq!(
        requests[0].headers.get("authorization").map(String::as_str),
        Some("token test-token")
    );
    assert!(requests[1].body.contains("userName=alice"));
    assert!(requests[1].body.contains("password=secret-pass"));
    assert_eq!(
        requests[2].headers.get("cookie").map(String::as_str),
        Some("JSESSIONID=session123")
    );
    assert!(requests[2].body.contains("issueId=7"));
    assert!(requests[2].body.contains("action=close"));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Closed issue #7"));
}

#[test]
fn issue_reopen_falls_back_to_gitbucket_web_session() {
    let env = GbTestEnv::new();
    let (port, server) = spawn_scripted_server(vec![
        ScriptedResponse::json(
            "PATCH /gitbucket/api/v3/repos/alice/project/issues/8 HTTP/1.1",
            "404 Not Found",
            r#"{"message":"Not Found"}"#,
        ),
        ScriptedResponse::html("POST /gitbucket/signin HTTP/1.1", "200 OK", "signed in")
            .with_header("set-cookie", "JSESSIONID=session234; Path=/; HttpOnly"),
        ScriptedResponse::html(
            "POST /gitbucket/alice/project/issue_comments/state HTTP/1.1",
            "200 OK",
            "updated",
        ),
    ]);

    let output = env
        .repo_api_command(format!("127.0.0.1:{port}/gitbucket"), "alice/project")
        .env("GB_USER", "alice")
        .env("GB_PASSWORD", "secret-pass")
        .args(["issue", "reopen", "8"])
        .output()
        .unwrap();

    let requests = server.join().unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(requests.len(), 3);
    assert_eq!(
        requests[2].headers.get("cookie").map(String::as_str),
        Some("JSESSIONID=session234")
    );
    assert!(requests[2].body.contains("issueId=8"));
    assert!(requests[2].body.contains("action=reopen"));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Reopened issue #8"));
}
