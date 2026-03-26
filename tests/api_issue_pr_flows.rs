mod support;

use serde_json::Value;
use tempfile::tempdir;

use support::gb_cmd::gb_command;
use support::mock_http::{spawn_scripted_server, spawn_server, ScriptedResponse};

#[test]
fn issue_create_sends_labels_assignees_and_body() {
    let temp = tempdir().unwrap();
    let (port, server) = spawn_server(
        "200 OK",
        r#"{"number":7,"title":"Bug report","state":"open","labels":[],"assignees":[]}"#,
    );

    let output = gb_command()
        .current_dir(temp.path())
        .env("GB_CONFIG_DIR", temp.path())
        .env("GB_HOST", format!("127.0.0.1:{port}"))
        .env("GB_REPO", "alice/project")
        .env("GB_TOKEN", "test-token")
        .env("GB_PROTOCOL", "http")
        .args([
            "issue",
            "create",
            "-t",
            "Bug report",
            "-b",
            "Body text",
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
    assert_eq!(body["title"], "Bug report");
    assert_eq!(body["body"], "Body text");
    assert_eq!(body["labels"], serde_json::json!(["bug", "urgent"]));
    assert_eq!(body["assignees"], serde_json::json!(["alice", "bob"]));
}

#[test]
fn pr_create_sends_expected_payload() {
    let temp = tempdir().unwrap();
    let (port, server) = spawn_server(
        "200 OK",
        r#"{"number":5,"title":"Add feature","state":"open"}"#,
    );

    let output = gb_command()
        .current_dir(temp.path())
        .env("GB_CONFIG_DIR", temp.path())
        .env("GB_HOST", format!("127.0.0.1:{port}"))
        .env("GB_REPO", "alice/project")
        .env("GB_TOKEN", "test-token")
        .env("GB_PROTOCOL", "http")
        .args([
            "pr",
            "create",
            "-t",
            "Add feature",
            "-b",
            "PR body",
            "--head",
            "feature/branch",
            "--base",
            "main",
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
    assert_eq!(request.target, "/api/v3/repos/alice/project/pulls");
    let body: Value = serde_json::from_str(&request.body).unwrap();
    assert_eq!(body["title"], "Add feature");
    assert_eq!(body["body"], "PR body");
    assert_eq!(body["head"], "feature/branch");
    assert_eq!(body["base"], "main");
}

#[test]
fn issue_close_sends_closed_state_patch() {
    let temp = tempdir().unwrap();
    let (port, server) = spawn_server(
        "200 OK",
        r#"{"number":7,"title":"Bug report","state":"closed","labels":[]}"#,
    );

    let output = gb_command()
        .current_dir(temp.path())
        .env("GB_CONFIG_DIR", temp.path())
        .env("GB_HOST", format!("127.0.0.1:{port}"))
        .env("GB_REPO", "alice/project")
        .env("GB_TOKEN", "test-token")
        .env("GB_PROTOCOL", "http")
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
fn pr_close_sends_closed_state_patch() {
    let temp = tempdir().unwrap();
    let (port, server) = spawn_server(
        "200 OK",
        r#"{"number":9,"title":"Feature","state":"closed","labels":[]}"#,
    );

    let output = gb_command()
        .current_dir(temp.path())
        .env("GB_CONFIG_DIR", temp.path())
        .env("GB_HOST", format!("127.0.0.1:{port}"))
        .env("GB_REPO", "alice/project")
        .env("GB_TOKEN", "test-token")
        .env("GB_PROTOCOL", "http")
        .args(["pr", "close", "9"])
        .output()
        .unwrap();

    let request = server.join().unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(request.method, "PATCH");
    assert_eq!(request.target, "/api/v3/repos/alice/project/issues/9");
    let body: Value = serde_json::from_str(&request.body).unwrap();
    assert_eq!(body["state"], "closed");
    assert!(body["title"].is_null());
    assert!(body["body"].is_null());
}

#[test]
fn pr_merge_sends_expected_payload() {
    let temp = tempdir().unwrap();
    let (port, server) = spawn_server("200 OK", r#"{"merged":true,"message":"merged"}"#);

    let output = gb_command()
        .current_dir(temp.path())
        .env("GB_CONFIG_DIR", temp.path())
        .env("GB_HOST", format!("127.0.0.1:{port}"))
        .env("GB_REPO", "alice/project")
        .env("GB_TOKEN", "test-token")
        .env("GB_PROTOCOL", "http")
        .args(["pr", "merge", "5", "--message", "Ship it"])
        .output()
        .unwrap();

    let request = server.join().unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(request.method, "PUT");
    assert_eq!(request.target, "/api/v3/repos/alice/project/pulls/5/merge");
    let body: Value = serde_json::from_str(&request.body).unwrap();
    assert_eq!(body["commit_message"], "Ship it");
    assert!(body["sha"].is_null());
    assert!(body["merge_method"].is_null());
}

#[test]
fn issue_reopen_sends_open_state_patch() {
    let temp = tempdir().unwrap();
    let (port, server) = spawn_server(
        "200 OK",
        r#"{"number":7,"title":"Bug report","state":"open","labels":[]}"#,
    );

    let output = gb_command()
        .current_dir(temp.path())
        .env("GB_CONFIG_DIR", temp.path())
        .env("GB_HOST", format!("127.0.0.1:{port}"))
        .env("GB_REPO", "alice/project")
        .env("GB_TOKEN", "test-token")
        .env("GB_PROTOCOL", "http")
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
    let temp = tempdir().unwrap();
    let (port, server) = spawn_server("200 OK", r#"{"id":11,"body":"Looks good"}"#);

    let output = gb_command()
        .current_dir(temp.path())
        .env("GB_CONFIG_DIR", temp.path())
        .env("GB_HOST", format!("127.0.0.1:{port}"))
        .env("GB_REPO", "alice/project")
        .env("GB_TOKEN", "test-token")
        .env("GB_PROTOCOL", "http")
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
fn pr_comment_sends_expected_payload() {
    let temp = tempdir().unwrap();
    let (port, server) = spawn_server("200 OK", r#"{"id":12,"body":"Please rebase"}"#);

    let output = gb_command()
        .current_dir(temp.path())
        .env("GB_CONFIG_DIR", temp.path())
        .env("GB_HOST", format!("127.0.0.1:{port}"))
        .env("GB_REPO", "alice/project")
        .env("GB_TOKEN", "test-token")
        .env("GB_PROTOCOL", "http")
        .args(["pr", "comment", "5", "--body", "Please rebase"])
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
        "/api/v3/repos/alice/project/issues/5/comments"
    );
    let body: Value = serde_json::from_str(&request.body).unwrap();
    assert_eq!(body["body"], "Please rebase");
}

#[test]
fn pr_create_accepts_gitbucket_wrapped_json_response() {
    let temp = tempdir().unwrap();
    let (port, server) = spawn_server(
        "200 OK",
        r#""{\"number\":9,\"title\":\"Wrapped PR\",\"state\":\"open\",\"merged\":false,\"head\":{\"ref\":\"feature/demo\"},\"base\":{\"ref\":\"main\"}}""#,
    );

    let output = gb_command()
        .current_dir(temp.path())
        .env("GB_CONFIG_DIR", temp.path())
        .env("GB_HOST", format!("127.0.0.1:{port}"))
        .env("GB_REPO", "alice/project")
        .env("GB_TOKEN", "test-token")
        .env("GB_PROTOCOL", "http")
        .args([
            "pr",
            "create",
            "-t",
            "Wrapped PR",
            "-b",
            "Body",
            "--head",
            "feature/demo",
            "--base",
            "main",
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
    assert_eq!(request.target, "/api/v3/repos/alice/project/pulls");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Created pull request #9"));
}

#[test]
fn pr_merge_accepts_gitbucket_enveloped_response() {
    let temp = tempdir().unwrap();
    let (port, server) = spawn_server(
        "200 OK",
        r#"{"status":200,"body":"{\"sha\":\"abc123\",\"merged\":true,\"message\":\"Pull Request successfully merged\"}","headers":{}}"#,
    );

    let output = gb_command()
        .current_dir(temp.path())
        .env("GB_CONFIG_DIR", temp.path())
        .env("GB_HOST", format!("127.0.0.1:{port}"))
        .env("GB_REPO", "alice/project")
        .env("GB_TOKEN", "test-token")
        .env("GB_PROTOCOL", "http")
        .args(["pr", "merge", "9", "-m", "merge body"])
        .output()
        .unwrap();

    let request = server.join().unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(request.method, "PUT");
    assert_eq!(request.target, "/api/v3/repos/alice/project/pulls/9/merge");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Merged pull request #9"));
}

#[test]
fn issue_close_falls_back_to_gitbucket_web_session() {
    let temp = tempdir().unwrap();
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

    let output = gb_command()
        .current_dir(temp.path())
        .env("GB_CONFIG_DIR", temp.path())
        .env("GB_HOST", format!("127.0.0.1:{port}/gitbucket"))
        .env("GB_REPO", "alice/project")
        .env("GB_TOKEN", "test-token")
        .env("GB_PROTOCOL", "http")
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
    let temp = tempdir().unwrap();
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

    let output = gb_command()
        .current_dir(temp.path())
        .env("GB_CONFIG_DIR", temp.path())
        .env("GB_HOST", format!("127.0.0.1:{port}/gitbucket"))
        .env("GB_REPO", "alice/project")
        .env("GB_TOKEN", "test-token")
        .env("GB_PROTOCOL", "http")
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
