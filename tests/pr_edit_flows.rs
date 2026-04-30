mod support;

use serde_json::Value;
use tempfile::tempdir;

use support::gb_cmd::gb_command;
use support::mock_http::{spawn_scripted_server, ScriptedResponse};

#[test]
fn pr_edit_updates_title_body_state_and_assignees() {
    let temp = tempdir().unwrap();
    let (port, server) = spawn_scripted_server(vec![
        ScriptedResponse::json(
            "GET /api/v3/repos/alice/project/pulls/5 HTTP/1.1",
            "200 OK",
            r#"{"number":5,"title":"Old","state":"open"}"#,
        ),
        ScriptedResponse::json(
            "GET /api/v3/repos/alice/project/issues/5 HTTP/1.1",
            "200 OK",
            r#"{"number":5,"title":"Old","state":"open","labels":[],"assignees":[{"login":"alice"}]}"#,
        ),
        ScriptedResponse::json(
            "PATCH /api/v3/repos/alice/project/issues/5 HTTP/1.1",
            "200 OK",
            r#"{"number":5,"title":"New","state":"closed","labels":[],"assignees":[{"login":"bob"}]}"#,
        ),
    ]);

    let output = gb_command()
        .current_dir(temp.path())
        .env("GB_CONFIG_DIR", temp.path())
        .env("GB_HOST", format!("127.0.0.1:{port}"))
        .env("GB_REPO", "alice/project")
        .env("GB_TOKEN", "test-token")
        .env("GB_PROTOCOL", "http")
        .args([
            "pr",
            "edit",
            "5",
            "--title",
            "New",
            "--body",
            "Updated body",
            "--add-assignee",
            "bob",
            "--remove-assignee",
            "alice",
            "--state",
            "closed",
        ])
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
    assert_eq!(requests[0].target, "/api/v3/repos/alice/project/pulls/5");
    assert_eq!(requests[1].method, "GET");
    assert_eq!(requests[1].target, "/api/v3/repos/alice/project/issues/5");
    assert_eq!(requests[2].method, "PATCH");
    assert_eq!(requests[2].target, "/api/v3/repos/alice/project/issues/5");
    let body: Value = serde_json::from_str(&requests[2].body).unwrap();
    assert_eq!(body["title"], "New");
    assert_eq!(body["body"], "Updated body");
    assert_eq!(body["state"], "closed");
    assert_eq!(body["assignees"], serde_json::json!(["bob"]));
}

#[test]
fn pr_edit_fails_non_interactively_when_issue_patch_is_missing() {
    let temp = tempdir().unwrap();
    let (port, server) = spawn_scripted_server(vec![
        ScriptedResponse::json(
            "GET /gitbucket/api/v3/repos/alice/project/pulls/5 HTTP/1.1",
            "200 OK",
            r#"{"number":5,"title":"Old","body":"Old body","state":"open"}"#,
        ),
        ScriptedResponse::json(
            "GET /gitbucket/api/v3/repos/alice/project/issues/5 HTTP/1.1",
            "200 OK",
            r#"{"number":5,"title":"Old","body":"Old body","state":"open","labels":[],"assignees":[{"login":"alice"}]}"#,
        ),
        ScriptedResponse::json(
            "PATCH /gitbucket/api/v3/repos/alice/project/issues/5 HTTP/1.1",
            "404 Not Found",
            r#"{"message":"Not Found"}"#,
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
        .args(["pr", "edit", "5", "--title", "New", "--add-assignee", "bob"])
        .output()
        .unwrap();

    let requests = server.join().unwrap();

    assert!(!output.status.success());
    assert_eq!(requests.len(), 3);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Re-run with --web to allow the GitBucket web UI fallback"),
        "stderr: {stderr}"
    );
    assert!(!stderr.contains("using web fallback"), "stderr: {stderr}");
}

#[test]
fn json_errors_suppresses_notice_before_failure() {
    let temp = tempdir().unwrap();
    let (port, server) = spawn_scripted_server(vec![
        ScriptedResponse::json(
            "GET /api/v3/repos/alice/project/pulls/5 HTTP/1.1",
            "200 OK",
            r#"{"number":5,"title":"Old","body":"","state":"open"}"#,
        ),
        ScriptedResponse::json(
            "PATCH /api/v3/repos/alice/project/issues/5 HTTP/1.1",
            "404 Not Found",
            r#"{"message":"Not Found"}"#,
        ),
    ]);

    let output = gb_command()
        .current_dir(temp.path())
        .env("GB_CONFIG_DIR", temp.path())
        .env("GB_HOST", format!("127.0.0.1:{port}"))
        .env("GB_REPO", "alice/project")
        .env("GB_TOKEN", "test-token")
        .env("GB_PROTOCOL", "http")
        .args([
            "--json-errors",
            "pr",
            "edit",
            "5",
            "--title",
            "New",
            "--web",
        ])
        .output()
        .unwrap();

    let requests = server.join().unwrap();

    assert_eq!(requests.len(), 2);
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    let value: Value = serde_json::from_str(stderr.trim()).unwrap();
    assert_eq!(value["error"]["code"], "auth_error");
    assert!(!stderr.contains("Notice:"), "stderr: {stderr}");
}

#[test]
fn pr_edit_uses_gitbucket_web_session_with_web_flag_when_issue_patch_is_missing() {
    let temp = tempdir().unwrap();
    let (port, server) = spawn_scripted_server(vec![
        ScriptedResponse::json(
            "GET /gitbucket/api/v3/repos/alice/project/pulls/5 HTTP/1.1",
            "200 OK",
            r#"{"number":5,"title":"Old","body":"Old body","state":"open"}"#,
        ),
        ScriptedResponse::json(
            "GET /gitbucket/api/v3/repos/alice/project/issues/5 HTTP/1.1",
            "200 OK",
            r#"{"number":5,"title":"Old","body":"Old body","state":"open","labels":[],"assignees":[{"login":"alice"}]}"#,
        ),
        ScriptedResponse::json(
            "PATCH /gitbucket/api/v3/repos/alice/project/issues/5 HTTP/1.1",
            "404 Not Found",
            r#"{"message":"Not Found"}"#,
        ),
        ScriptedResponse::html("POST /gitbucket/signin HTTP/1.1", "200 OK", "signed in")
            .with_header("set-cookie", "JSESSIONID=session347; Path=/; HttpOnly"),
        ScriptedResponse::html(
            "POST /gitbucket/alice/project/issues/edit_title/5 HTTP/1.1",
            "200 OK",
            "title",
        ),
        ScriptedResponse::html(
            "POST /gitbucket/alice/project/issues/edit/5 HTTP/1.1",
            "200 OK",
            "content",
        ),
        ScriptedResponse::html(
            "POST /gitbucket/alice/project/issues/5/assignee/delete HTTP/1.1",
            "200 OK",
            "removed",
        ),
        ScriptedResponse::html(
            "POST /gitbucket/alice/project/issues/5/assignee/new HTTP/1.1",
            "200 OK",
            "added",
        ),
        ScriptedResponse::html(
            "POST /gitbucket/alice/project/issue_comments/state HTTP/1.1",
            "200 OK",
            "closed",
        ),
        ScriptedResponse::json(
            "GET /gitbucket/api/v3/repos/alice/project/pulls/5 HTTP/1.1",
            "200 OK",
            r#"{"number":5,"title":"New","body":"Updated body","state":"closed"}"#,
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
        .args([
            "pr",
            "edit",
            "5",
            "--title",
            "New",
            "--body",
            "Updated body",
            "--add-assignee",
            "bob",
            "--remove-assignee",
            "alice",
            "--state",
            "closed",
            "--web",
        ])
        .output()
        .unwrap();

    let requests = server.join().unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("using web fallback"),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(requests.len(), 10);
    assert!(requests[3].body.contains("userName=alice"));
    assert_eq!(
        requests[4].headers.get("cookie").map(String::as_str),
        Some("JSESSIONID=session347")
    );
    assert!(requests[4].body.contains("title=New"));
    assert!(requests[5].body.contains("title=New"));
    assert!(requests[5].body.contains("content=Updated+body"));
    assert!(requests[6].body.contains("assigneeUserName=alice"));
    assert!(requests[7].body.contains("assigneeUserName=bob"));
    assert!(requests[8].body.contains("issueId=5"));
    assert!(requests[8].body.contains("action=close"));
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("Updated pull request #5: New"),
        "stdout: {}",
        String::from_utf8_lossy(&output.stdout)
    );
}

#[test]
fn pr_edit_rejects_issue_number_that_is_not_a_pull_request() {
    let temp = tempdir().unwrap();
    let (port, server) = spawn_scripted_server(vec![ScriptedResponse::json(
        "GET /api/v3/repos/alice/project/pulls/5 HTTP/1.1",
        "404 Not Found",
        r#"{"message":"not found"}"#,
    )]);

    let output = gb_command()
        .current_dir(temp.path())
        .env("GB_CONFIG_DIR", temp.path())
        .env("GB_HOST", format!("127.0.0.1:{port}"))
        .env("GB_REPO", "alice/project")
        .env("GB_TOKEN", "test-token")
        .env("GB_PROTOCOL", "http")
        .args(["pr", "edit", "5", "--title", "Wrong target"])
        .output()
        .unwrap();

    let requests = server.join().unwrap();

    assert!(!output.status.success());
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].method, "GET");
    assert_eq!(requests[0].target, "/api/v3/repos/alice/project/pulls/5");
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("API error (404)"),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}
