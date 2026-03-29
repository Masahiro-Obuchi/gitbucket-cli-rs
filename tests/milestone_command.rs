mod support;

use serde_json::Value;
use tempfile::tempdir;

use support::gb_cmd::gb_command;
use support::mock_http::{spawn_scripted_server, spawn_server, ScriptedResponse};

#[test]
fn milestone_list_prints_json_and_state_query() {
    let temp = tempdir().unwrap();
    let (port, server) = spawn_server(
        "200 OK",
        r#"[{"number":7,"title":"v1.0","state":"open","description":"First release","open_issues":3,"closed_issues":1,"due_on":"2026-04-01T00:00:00Z"}]"#,
    );

    let output = gb_command()
        .current_dir(temp.path())
        .env("GB_CONFIG_DIR", temp.path())
        .env("GB_HOST", format!("127.0.0.1:{port}"))
        .env("GB_REPO", "alice/project")
        .env("GB_TOKEN", "test-token")
        .env("GB_PROTOCOL", "http")
        .args(["milestone", "list", "--state", "all", "--json"])
        .output()
        .unwrap();

    let request = server.join().unwrap();

    assert!(output.status.success());
    assert_eq!(request.method, "GET");
    assert_eq!(
        request.target,
        "/api/v3/repos/alice/project/milestones?state=all"
    );
    let payload: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(payload[0]["title"], "v1.0");
}

#[test]
fn milestone_view_prints_details() {
    let temp = tempdir().unwrap();
    let (port, server) = spawn_server(
        "200 OK",
        r#"{"number":7,"title":"v1.0","state":"open","description":"First release","open_issues":3,"closed_issues":1,"due_on":"2026-04-01T00:00:00Z","html_url":"http://example.test/alice/project/milestone/7"}"#,
    );

    let output = gb_command()
        .current_dir(temp.path())
        .env("GB_CONFIG_DIR", temp.path())
        .env("GB_HOST", format!("127.0.0.1:{port}"))
        .env("GB_REPO", "alice/project")
        .env("GB_TOKEN", "test-token")
        .env("GB_PROTOCOL", "http")
        .args(["milestone", "view", "7"])
        .output()
        .unwrap();

    let request = server.join().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(output.status.success());
    assert_eq!(request.method, "GET");
    assert_eq!(request.target, "/api/v3/repos/alice/project/milestones/7");
    assert!(stdout.contains("v1.0"), "stdout: {stdout}");
    assert!(
        stdout.contains("Due: 2026-04-01T00:00:00Z"),
        "stdout: {stdout}"
    );
    assert!(stdout.contains("Open issues: 3"), "stdout: {stdout}");
}

#[test]
fn milestone_view_hides_unset_due_date_sentinel() {
    let temp = tempdir().unwrap();
    let (port, server) = spawn_server(
        "200 OK",
        r#"{"number":7,"title":"v1.0","state":"open","description":"First release","open_issues":3,"closed_issues":1,"due_on":"0001-01-01T00:00:00Z"}"#,
    );

    let output = gb_command()
        .current_dir(temp.path())
        .env("GB_CONFIG_DIR", temp.path())
        .env("GB_HOST", format!("127.0.0.1:{port}"))
        .env("GB_REPO", "alice/project")
        .env("GB_TOKEN", "test-token")
        .env("GB_PROTOCOL", "http")
        .args(["milestone", "view", "7"])
        .output()
        .unwrap();

    let request = server.join().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(output.status.success());
    assert_eq!(request.method, "GET");
    assert_eq!(request.target, "/api/v3/repos/alice/project/milestones/7");
    assert!(!stdout.contains("Due:"), "stdout: {stdout}");
}

#[test]
fn milestone_create_sends_expected_payload() {
    let temp = tempdir().unwrap();
    let (port, server) = spawn_server(
        "201 Created",
        r#"{"number":7,"title":"v1.0","state":"open","description":"First release","due_on":"2026-04-01T00:00:00Z"}"#,
    );

    let output = gb_command()
        .current_dir(temp.path())
        .env("GB_CONFIG_DIR", temp.path())
        .env("GB_HOST", format!("127.0.0.1:{port}"))
        .env("GB_REPO", "alice/project")
        .env("GB_TOKEN", "test-token")
        .env("GB_PROTOCOL", "http")
        .args([
            "milestone",
            "create",
            "v1.0",
            "--description",
            "First release",
            "--due-on",
            "2026-04-01",
        ])
        .output()
        .unwrap();

    let request = server.join().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(request.method, "POST");
    assert_eq!(request.target, "/api/v3/repos/alice/project/milestones");
    let body: Value = serde_json::from_str(&request.body).unwrap();
    assert_eq!(body["title"], "v1.0");
    assert_eq!(body["description"], "First release");
    assert_eq!(body["due_on"], "2026-04-01T00:00:00Z");
    assert!(stdout.contains("Created milestone #7"), "stdout: {stdout}");
}

#[test]
fn milestone_create_rejects_invalid_due_on_before_api_call() {
    let temp = tempdir().unwrap();

    let output = gb_command()
        .current_dir(temp.path())
        .env("GB_CONFIG_DIR", temp.path())
        .env("GB_HOST", "127.0.0.1:9")
        .env("GB_REPO", "alice/project")
        .env("GB_TOKEN", "test-token")
        .env("GB_PROTOCOL", "http")
        .args(["milestone", "create", "v1.0", "--due-on", "tomorrow"])
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .contains("Invalid due date 'tomorrow'. Expected YYYY-MM-DD or RFC3339."),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn milestone_create_falls_back_to_gitbucket_web_session() {
    let temp = tempdir().unwrap();
    let (port, server) = spawn_scripted_server(vec![
        ScriptedResponse::json(
            "POST /gitbucket/api/v3/repos/alice/project/milestones HTTP/1.1",
            "404 Not Found",
            r#"{"message":"Not Found"}"#,
        ),
        ScriptedResponse::html("POST /gitbucket/signin HTTP/1.1", "200 OK", "signed in")
            .with_header("set-cookie", "JSESSIONID=session345; Path=/; HttpOnly"),
        ScriptedResponse::html(
            "POST /gitbucket/alice/project/issues/milestones/new HTTP/1.1",
            "200 OK",
            "created",
        ),
        ScriptedResponse::json(
            "GET /gitbucket/api/v3/repos/alice/project/milestones?state=all HTTP/1.1",
            "200 OK",
            r#"[{"number":7,"title":"v1.0","state":"open"}]"#,
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
            "milestone",
            "create",
            "v1.0",
            "--description",
            "First release",
            "--due-on",
            "2026-04-01",
        ])
        .output()
        .unwrap();

    let requests = server.join().unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(requests.len(), 4);
    assert!(requests[1].body.contains("userName=alice"));
    assert_eq!(
        requests[2].headers.get("cookie").map(String::as_str),
        Some("JSESSIONID=session345")
    );
    assert!(requests[2].body.contains("title=v1.0"));
    assert!(requests[2].body.contains("description=First+release"));
    assert!(requests[2].body.contains("dueDate=2026-04-01"));
}

#[test]
fn milestone_create_web_fallback_succeeds_when_follow_up_list_fails() {
    let temp = tempdir().unwrap();
    let (port, server) = spawn_scripted_server(vec![
        ScriptedResponse::json(
            "POST /gitbucket/api/v3/repos/alice/project/milestones HTTP/1.1",
            "404 Not Found",
            r#"{"message":"Not Found"}"#,
        ),
        ScriptedResponse::html("POST /gitbucket/signin HTTP/1.1", "200 OK", "signed in")
            .with_header("set-cookie", "JSESSIONID=session346; Path=/; HttpOnly"),
        ScriptedResponse::html(
            "POST /gitbucket/alice/project/issues/milestones/new HTTP/1.1",
            "200 OK",
            "created",
        ),
        ScriptedResponse::json(
            "GET /gitbucket/api/v3/repos/alice/project/milestones?state=all HTTP/1.1",
            "401 Unauthorized",
            r#"{"message":"Bad credentials"}"#,
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
        .args(["milestone", "create", "v1.0"])
        .output()
        .unwrap();

    let requests = server.join().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(requests.len(), 4);
    assert!(stdout.contains("Created milestone v1.0"), "stdout: {stdout}");
}

#[test]
fn milestone_edit_requires_an_explicit_change() {
    let temp = tempdir().unwrap();

    let output = gb_command()
        .current_dir(temp.path())
        .env("GB_CONFIG_DIR", temp.path())
        .env("GB_HOST", "127.0.0.1:9")
        .env("GB_REPO", "alice/project")
        .env("GB_TOKEN", "test-token")
        .env("GB_PROTOCOL", "http")
        .args(["milestone", "edit", "7"])
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("No milestone changes requested."),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn milestone_edit_sends_expected_payload() {
    let temp = tempdir().unwrap();
    let (port, server) = spawn_scripted_server(vec![
        ScriptedResponse::json(
            "GET /api/v3/repos/alice/project/milestones/7 HTTP/1.1",
            "200 OK",
            r#"{"number":7,"title":"v1.0","state":"open","description":"First release"}"#,
        ),
        ScriptedResponse::json(
            "PATCH /api/v3/repos/alice/project/milestones/7 HTTP/1.1",
            "200 OK",
            r#"{"number":7,"title":"v1.1","state":"closed","description":"Updated","due_on":"2026-04-02T00:00:00Z"}"#,
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
            "milestone",
            "edit",
            "7",
            "--title",
            "v1.1",
            "--description",
            "Updated",
            "--due-on",
            "2026-04-02T09:30:00Z",
            "--state",
            "closed",
        ])
        .output()
        .unwrap();

    let requests = server.join().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(requests.len(), 2);
    let body: Value = serde_json::from_str(&requests[1].body).unwrap();
    assert_eq!(body["title"], "v1.1");
    assert_eq!(body["description"], "Updated");
    assert_eq!(body["due_on"], "2026-04-02T00:00:00Z");
    assert_eq!(body["state"], "closed");
    assert!(stdout.contains("Updated milestone #7"), "stdout: {stdout}");
}

#[test]
fn milestone_edit_falls_back_to_gitbucket_web_session() {
    let temp = tempdir().unwrap();
    let (port, server) = spawn_scripted_server(vec![
        ScriptedResponse::json(
            "GET /gitbucket/api/v3/repos/alice/project/milestones/7 HTTP/1.1",
            "200 OK",
            r#"{"number":7,"title":"v1.0","state":"open","description":"First release","due_on":"2026-04-01T00:00:00Z"}"#,
        ),
        ScriptedResponse::json(
            "PATCH /gitbucket/api/v3/repos/alice/project/milestones/7 HTTP/1.1",
            "404 Not Found",
            r#"{"message":"Not Found"}"#,
        ),
        ScriptedResponse::html("POST /gitbucket/signin HTTP/1.1", "200 OK", "signed in")
            .with_header("set-cookie", "JSESSIONID=session456; Path=/; HttpOnly"),
        ScriptedResponse::html(
            "POST /gitbucket/alice/project/issues/milestones/7/edit HTTP/1.1",
            "200 OK",
            "updated",
        ),
        ScriptedResponse::html(
            "GET /gitbucket/alice/project/issues/milestones/7/close HTTP/1.1",
            "200 OK",
            "closed",
        ),
        ScriptedResponse::json(
            "GET /gitbucket/api/v3/repos/alice/project/milestones/7 HTTP/1.1",
            "200 OK",
            r#"{"number":7,"title":"v1.1","state":"closed","description":"Updated","due_on":"2026-04-02T00:00:00Z"}"#,
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
            "milestone",
            "edit",
            "7",
            "--title",
            "v1.1",
            "--description",
            "Updated",
            "--due-on",
            "2026-04-02",
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
    assert_eq!(requests.len(), 6);
    assert_eq!(
        requests[3].headers.get("cookie").map(String::as_str),
        Some("JSESSIONID=session456")
    );
    assert!(requests[3].body.contains("title=v1.1"));
    assert!(requests[3].body.contains("description=Updated"));
    assert!(requests[3].body.contains("dueDate=2026-04-02"));
    assert_eq!(
        requests[4].headers.get("cookie").map(String::as_str),
        Some("JSESSIONID=session456")
    );
}

#[test]
fn milestone_edit_fallback_keeps_unset_due_date_empty() {
    let temp = tempdir().unwrap();
    let (port, server) = spawn_scripted_server(vec![
        ScriptedResponse::json(
            "GET /gitbucket/api/v3/repos/alice/project/milestones/7 HTTP/1.1",
            "200 OK",
            r#"{"number":7,"title":"v1.0","state":"open","description":"First release","due_on":"0001-01-01T00:00:00Z"}"#,
        ),
        ScriptedResponse::json(
            "PATCH /gitbucket/api/v3/repos/alice/project/milestones/7 HTTP/1.1",
            "404 Not Found",
            r#"{"message":"Not Found"}"#,
        ),
        ScriptedResponse::html("POST /gitbucket/signin HTTP/1.1", "200 OK", "signed in")
            .with_header("set-cookie", "JSESSIONID=session789; Path=/; HttpOnly"),
        ScriptedResponse::html(
            "POST /gitbucket/alice/project/issues/milestones/7/edit HTTP/1.1",
            "200 OK",
            "updated",
        ),
        ScriptedResponse::json(
            "GET /gitbucket/api/v3/repos/alice/project/milestones/7 HTTP/1.1",
            "200 OK",
            r#"{"number":7,"title":"v1.1","state":"open","description":"First release","due_on":"0001-01-01T00:00:00Z"}"#,
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
        .args(["milestone", "edit", "7", "--title", "v1.1"])
        .output()
        .unwrap();

    let requests = server.join().unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(requests.len(), 5);
    assert!(requests[3].body.contains("title=v1.1"));
    assert!(requests[3].body.contains("dueDate="));
    assert!(!requests[3].body.contains("dueDate=0001-01-01"));
}

#[test]
fn milestone_delete_sends_delete_request_when_yes_is_used() {
    let temp = tempdir().unwrap();
    let (port, server) = spawn_server("204 No Content", "");

    let output = gb_command()
        .current_dir(temp.path())
        .env("GB_CONFIG_DIR", temp.path())
        .env("GB_HOST", format!("127.0.0.1:{port}"))
        .env("GB_REPO", "alice/project")
        .env("GB_TOKEN", "test-token")
        .env("GB_PROTOCOL", "http")
        .args(["milestone", "delete", "7", "--yes"])
        .output()
        .unwrap();

    let request = server.join().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(request.method, "DELETE");
    assert_eq!(request.target, "/api/v3/repos/alice/project/milestones/7");
    assert!(stdout.contains("Deleted milestone #7"), "stdout: {stdout}");
}

#[test]
fn milestone_delete_falls_back_to_gitbucket_web_session() {
    let temp = tempdir().unwrap();
    let (port, server) = spawn_scripted_server(vec![
        ScriptedResponse::json(
            "DELETE /gitbucket/api/v3/repos/alice/project/milestones/7 HTTP/1.1",
            "404 Not Found",
            r#"{"message":"Not Found"}"#,
        ),
        ScriptedResponse::html("POST /gitbucket/signin HTTP/1.1", "200 OK", "signed in")
            .with_header("set-cookie", "JSESSIONID=session567; Path=/; HttpOnly"),
        ScriptedResponse::html(
            "GET /gitbucket/alice/project/issues/milestones/7/delete HTTP/1.1",
            "200 OK",
            "deleted",
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
        .args(["milestone", "delete", "7", "--yes"])
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
        Some("JSESSIONID=session567")
    );
}
