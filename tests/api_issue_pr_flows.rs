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
    let temp = tempdir().unwrap();
    let (port, server) = spawn_server(
        "200 OK",
        r#"{"number":8,"title":"Another bug","state":"open","labels":[],"assignees":[]}"#,
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
fn issue_edit_sends_expected_payload() {
    let temp = tempdir().unwrap();
    let (port, server) = spawn_scripted_server(vec![
        ScriptedResponse::json(
            "GET /api/v3/repos/alice/project/issues/7 HTTP/1.1",
            "200 OK",
            r#"{"number":7,"title":"Bug report","body":"Body text","state":"open","labels":[{"name":"bug"}],"assignees":[{"login":"alice"}],"milestone":{"number":3,"title":"v1.0","state":"open"}}"#,
        ),
        ScriptedResponse::json(
            "PATCH /api/v3/repos/alice/project/issues/7 HTTP/1.1",
            "200 OK",
            r#"{"number":7,"title":"Updated title","body":"Updated body","state":"closed","labels":[{"name":"urgent"}],"assignees":[{"login":"bob"}],"milestone":{"number":9,"title":"v2.0","state":"open"}}"#,
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
            "issue",
            "edit",
            "7",
            "--title",
            "Updated title",
            "--body",
            "Updated body",
            "--add-label",
            "urgent",
            "--remove-label",
            "bug",
            "--add-assignee",
            "bob",
            "--remove-assignee",
            "alice",
            "--milestone",
            "9",
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
    assert_eq!(requests.len(), 2);
    let patch = &requests[1];
    assert_eq!(patch.method, "PATCH");
    assert_eq!(patch.target, "/api/v3/repos/alice/project/issues/7");
    let body: Value = serde_json::from_str(&patch.body).unwrap();
    assert_eq!(body["title"], "Updated title");
    assert_eq!(body["body"], "Updated body");
    assert_eq!(body["state"], "closed");
    assert_eq!(body["labels"], serde_json::json!(["urgent"]));
    assert_eq!(body["assignees"], serde_json::json!(["bob"]));
    assert_eq!(body["milestone"], 9);
}

#[test]
fn issue_edit_can_clear_milestone() {
    let temp = tempdir().unwrap();
    let (port, server) = spawn_scripted_server(vec![
        ScriptedResponse::json(
            "GET /api/v3/repos/alice/project/issues/7 HTTP/1.1",
            "200 OK",
            r#"{"number":7,"title":"Bug report","state":"open","labels":[],"assignees":[],"milestone":{"number":3,"title":"v1.0","state":"open"}}"#,
        ),
        ScriptedResponse::json(
            "PATCH /api/v3/repos/alice/project/issues/7 HTTP/1.1",
            "200 OK",
            r#"{"number":7,"title":"Bug report","state":"open","labels":[],"assignees":[],"milestone":null}"#,
        ),
    ]);

    let output = gb_command()
        .current_dir(temp.path())
        .env("GB_CONFIG_DIR", temp.path())
        .env("GB_HOST", format!("127.0.0.1:{port}"))
        .env("GB_REPO", "alice/project")
        .env("GB_TOKEN", "test-token")
        .env("GB_PROTOCOL", "http")
        .args(["issue", "edit", "7", "--remove-milestone"])
        .output()
        .unwrap();

    let requests = server.join().unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let body: Value = serde_json::from_str(&requests[1].body).unwrap();
    assert!(body["milestone"].is_null());
}

#[test]
fn issue_edit_falls_back_to_gitbucket_web_session_for_supported_fields() {
    let temp = tempdir().unwrap();
    let (port, server) = spawn_scripted_server(vec![
        ScriptedResponse::json(
            "GET /gitbucket/api/v3/repos/alice/project/issues/7 HTTP/1.1",
            "200 OK",
            r#"{"number":7,"title":"Bug report","body":"Body text","state":"open","labels":[{"name":"bug"}],"assignees":[{"login":"alice"}],"milestone":{"number":3,"title":"v1.0","state":"open"}}"#,
        ),
        ScriptedResponse::json(
            "PATCH /gitbucket/api/v3/repos/alice/project/issues/7 HTTP/1.1",
            "404 Not Found",
            r#"{"message":"Not Found"}"#,
        ),
        ScriptedResponse::html("POST /gitbucket/signin HTTP/1.1", "200 OK", "signed in")
            .with_header("set-cookie", "JSESSIONID=session347; Path=/; HttpOnly"),
        ScriptedResponse::html(
            "POST /gitbucket/alice/project/issues/edit_title/7 HTTP/1.1",
            "200 OK",
            r#"{"title":"Updated title"}"#,
        ),
        ScriptedResponse::html(
            "POST /gitbucket/alice/project/issues/edit/7 HTTP/1.1",
            "200 OK",
            "edited",
        ),
        ScriptedResponse::html(
            "POST /gitbucket/alice/project/issues/7/milestone HTTP/1.1",
            "200 OK",
            "milestone",
        ),
        ScriptedResponse::html(
            "POST /gitbucket/alice/project/issue_comments/state HTTP/1.1",
            "200 OK",
            "closed",
        ),
        ScriptedResponse::json(
            "GET /gitbucket/api/v3/repos/alice/project/issues/7 HTTP/1.1",
            "200 OK",
            r#"{"number":7,"title":"Updated title","body":"Updated body","state":"closed","labels":[{"name":"bug"}],"assignees":[{"login":"alice"}],"milestone":{"number":9,"title":"v2.0","state":"open"}}"#,
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
            "issue",
            "edit",
            "7",
            "--title",
            "Updated title",
            "--body",
            "Updated body",
            "--milestone",
            "9",
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
    assert_eq!(requests.len(), 8);
    assert!(requests[2].body.contains("userName=alice"));
    assert_eq!(
        requests[3].headers.get("cookie").map(String::as_str),
        Some("JSESSIONID=session347")
    );
    assert!(requests[3].body.contains("title=Updated+title"));
    assert_eq!(
        requests[4].headers.get("cookie").map(String::as_str),
        Some("JSESSIONID=session347")
    );
    assert!(requests[4].body.contains("title=Updated+title"));
    assert!(requests[4].body.contains("content=Updated+body"));
    assert!(requests[5].body.contains("milestoneId=9"));
    assert!(requests[6].body.contains("issueId=7"));
    assert!(requests[6].body.contains("action=close"));
}

#[test]
fn issue_edit_rejects_label_and_assignee_changes_when_only_web_fallback_is_available() {
    let temp = tempdir().unwrap();
    let (port, server) = spawn_scripted_server(vec![
        ScriptedResponse::json(
            "GET /gitbucket/api/v3/repos/alice/project/issues/7 HTTP/1.1",
            "200 OK",
            r#"{"number":7,"title":"Bug report","body":"Body text","state":"open","labels":[{"name":"bug"}],"assignees":[{"login":"alice"}],"milestone":null}"#,
        ),
        ScriptedResponse::json(
            "PATCH /gitbucket/api/v3/repos/alice/project/issues/7 HTTP/1.1",
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
        .args(["issue", "edit", "7", "--add-label", "urgent"])
        .output()
        .unwrap();

    let requests = server.join().unwrap();

    assert!(!output.status.success());
    assert_eq!(requests.len(), 2);
    assert!(
        String::from_utf8_lossy(&output.stderr).contains(
            "does not support editing issue labels or assignees through the web fallback"
        ),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn issue_edit_requires_an_explicit_change() {
    let temp = tempdir().unwrap();

    let output = gb_command()
        .current_dir(temp.path())
        .env("GB_CONFIG_DIR", temp.path())
        .env("GB_HOST", "127.0.0.1:9")
        .env("GB_REPO", "alice/project")
        .env("GB_TOKEN", "test-token")
        .env("GB_PROTOCOL", "http")
        .args(["issue", "edit", "7"])
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("No issue changes requested."),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn issue_edit_rejects_conflicting_milestone_flags() {
    let temp = tempdir().unwrap();

    let output = gb_command()
        .current_dir(temp.path())
        .env("GB_CONFIG_DIR", temp.path())
        .env("GB_HOST", "127.0.0.1:9")
        .env("GB_REPO", "alice/project")
        .env("GB_TOKEN", "test-token")
        .env("GB_PROTOCOL", "http")
        .args([
            "issue",
            "edit",
            "7",
            "--milestone",
            "3",
            "--remove-milestone",
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .contains("Cannot use --milestone and --remove-milestone together."),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn issue_edit_rejects_invalid_state() {
    let temp = tempdir().unwrap();

    let output = gb_command()
        .current_dir(temp.path())
        .env("GB_CONFIG_DIR", temp.path())
        .env("GB_HOST", "127.0.0.1:9")
        .env("GB_REPO", "alice/project")
        .env("GB_TOKEN", "test-token")
        .env("GB_PROTOCOL", "http")
        .args(["issue", "edit", "7", "--state", "all"])
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .contains("invalid value 'all' for '--state <STATE>'"),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
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
fn pr_create_supports_head_owner_and_prints_resolved_refs() {
    let temp = tempdir().unwrap();
    let (port, server) = spawn_server(
        "200 OK",
        r#"{
            "number":5,
            "title":"Add feature",
            "state":"open",
            "html_url":"http://127.0.0.1/pulls/5",
            "head":{"ref":"feature/branch","repo":{"name":"project","full_name":"bob/project"}},
            "base":{"ref":"main","repo":{"name":"project","full_name":"alice/project"}}
        }"#,
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
            "--head-owner",
            "bob",
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
    let body: Value = serde_json::from_str(&request.body).unwrap();
    assert_eq!(body["head"], "bob:feature/branch");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Head: bob/project:feature/branch"));
    assert!(stdout.contains("Base: alice/project:main"));
    assert!(
        stdout.contains(&format!(
            "URL: http://127.0.0.1:{port}/alice/project/pull/5"
        )),
        "stdout: {stdout}"
    );
}

#[test]
fn pr_create_json_prints_created_pull_request() {
    let temp = tempdir().unwrap();
    let (port, server) = spawn_server(
        "200 OK",
        r#"{"number":5,"title":"Add feature","state":"open","head":{"ref":"feature/branch"},"base":{"ref":"main"}}"#,
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
            "--json",
        ])
        .output()
        .unwrap();

    let _request = server.join().unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(stdout["number"], 5);
    assert_eq!(stdout["head"]["ref"], "feature/branch");
    assert_eq!(stdout["base"]["ref"], "main");
}

#[test]
fn pr_edit_updates_title_body_state_and_assignees() {
    let temp = tempdir().unwrap();
    let (port, server) = spawn_scripted_server(vec![
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
    assert_eq!(requests.len(), 2);
    assert_eq!(requests[1].method, "PATCH");
    assert_eq!(requests[1].target, "/api/v3/repos/alice/project/issues/5");
    let body: Value = serde_json::from_str(&requests[1].body).unwrap();
    assert_eq!(body["title"], "New");
    assert_eq!(body["body"], "Updated body");
    assert_eq!(body["state"], "closed");
    assert_eq!(body["assignees"], serde_json::json!(["bob"]));
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
fn issue_comment_edit_last_updates_authenticated_users_latest_comment() {
    let temp = tempdir().unwrap();
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

    let output = gb_command()
        .current_dir(temp.path())
        .env("GB_CONFIG_DIR", temp.path())
        .env("GB_HOST", format!("127.0.0.1:{port}"))
        .env("GB_REPO", "alice/project")
        .env("GB_TOKEN", "test-token")
        .env("GB_PROTOCOL", "http")
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
    let temp = tempdir().unwrap();
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

    let output = gb_command()
        .current_dir(temp.path())
        .env("GB_CONFIG_DIR", temp.path())
        .env("GB_HOST", format!("127.0.0.1:{port}"))
        .env("GB_REPO", "alice/project")
        .env("GB_TOKEN", "test-token")
        .env("GB_PROTOCOL", "http")
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
    let temp = tempdir().unwrap();
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

    let output = gb_command()
        .current_dir(temp.path())
        .env("GB_CONFIG_DIR", temp.path())
        .env("GB_HOST", format!("127.0.0.1:{port}"))
        .env("GB_REPO", "alice/project")
        .env("GB_TOKEN", "test-token")
        .env("GB_PROTOCOL", "http")
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
fn pr_comment_edit_last_updates_authenticated_users_latest_comment() {
    let temp = tempdir().unwrap();
    let (port, server) = spawn_scripted_server(vec![
        ScriptedResponse::json(
            "GET /api/v3/user HTTP/1.1",
            "200 OK",
            r#"{"login":"alice"}"#,
        ),
        ScriptedResponse::json(
            "GET /api/v3/repos/alice/project/issues/5/comments?per_page=100 HTTP/1.1",
            "200 OK",
            r#"[
                {"id":10,"body":"Older","user":{"login":"alice"}},
                {"id":12,"body":"Latest","user":{"login":"alice"}}
            ]"#,
        ),
        ScriptedResponse::json(
            "PATCH /api/v3/repos/alice/project/issues/comments/12 HTTP/1.1",
            "200 OK",
            r#"{"id":12,"body":"Edited"}"#,
        ),
    ]);

    let output = gb_command()
        .current_dir(temp.path())
        .env("GB_CONFIG_DIR", temp.path())
        .env("GB_HOST", format!("127.0.0.1:{port}"))
        .env("GB_REPO", "alice/project")
        .env("GB_TOKEN", "test-token")
        .env("GB_PROTOCOL", "http")
        .args(["pr", "comment", "5", "--edit-last", "--body", "Edited"])
        .output()
        .unwrap();

    let requests = server.join().unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(requests.len(), 3);
    assert_eq!(requests[2].method, "PATCH");
    assert_eq!(
        requests[2].target,
        "/api/v3/repos/alice/project/issues/comments/12"
    );
    let body: Value = serde_json::from_str(&requests[2].body).unwrap();
    assert_eq!(body["body"], "Edited");
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("Edited comment 12 on PR #5"),
        "stdout: {}",
        String::from_utf8_lossy(&output.stdout)
    );
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
