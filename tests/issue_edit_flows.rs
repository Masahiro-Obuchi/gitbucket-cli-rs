mod support;

use serde_json::{json, Value};
use support::gb_cmd::GbTestEnv;
use support::gb_cmd::{assert_failure, assert_success, stderr};
use support::mock_http::{spawn_scripted_server, ScriptedResponse};

#[test]
fn issue_edit_separates_metadata_from_label_deltas_and_keeps_concurrent_labels() {
    let env = GbTestEnv::new();
    let before = initial_issue();
    let metadata = json!({"number":7,"title":"Updated title","body":"Updated body","state":"closed","labels":[{"name":"bug"}],"assignees":[{"login":"bob"}],"milestone":{"number":9,"title":"v2.0","state":"open"}});
    let mut after = metadata.clone();
    after["labels"] = json!([{"name":"urgent"},{"name":"concurrent"}]);
    let (port, server) = spawn_scripted_server(vec![
        item("GET", "", &before),
        repo_label("urgent"),
        item("PATCH", "", &metadata),
        item("GET", "", &metadata),
        labels_response(
            "GET",
            "",
            "",
            &json!([{"name":"bug"},{"name":"concurrent"}]),
        ),
        labels_response("DELETE", "", "/bug", &json!([])),
        labels_response("POST", "", "", &json!([{"name":"urgent"}])),
        labels_response(
            "GET",
            "",
            "",
            &json!([{"name":"urgent"},{"name":"concurrent"}]),
        ),
        item("GET", "", &after),
    ]);
    let output = env
        .repo_api_command(format!("127.0.0.1:{port}"), "alice/project")
        .env_remove("GB_PASSWORD")
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
    assert_success(&output);
    let requests = server.join().unwrap();
    let body: Value = serde_json::from_str(&requests[2].body).unwrap();
    assert!(body.get("labels").is_none());
    assert_eq!(body["assignees"], json!(["bob"]));
    assert_eq!(body["milestone"], 9);
    assert_eq!(
        serde_json::from_str::<Value>(&requests[6].body).unwrap(),
        json!({"labels":["urgent"]})
    );
    assert!(requests.iter().all(|request| request.method != "PUT"));
}

#[test]
fn issue_edit_can_clear_milestone() {
    let env = GbTestEnv::new();
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
        ScriptedResponse::json(
            "GET /api/v3/repos/alice/project/issues/7 HTTP/1.1",
            "200 OK",
            r#"{"number":7,"title":"Bug report","state":"open","labels":[],"assignees":[],"milestone":null}"#,
        ),
    ]);

    let output = env
        .repo_api_command(format!("127.0.0.1:{port}"), "alice/project")
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
    let env = GbTestEnv::new();
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

    let output = env
        .repo_api_command(format!("127.0.0.1:{port}/gitbucket"), "alice/project")
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
fn issue_edit_uses_web_fallback_for_assignee_changes() {
    let env = GbTestEnv::new();
    let (port, server) = spawn_scripted_server(vec![
        ScriptedResponse::json(
            "GET /gitbucket/api/v3/repos/alice/project/issues/7 HTTP/1.1",
            "200 OK",
            r#"{"number":7,"title":"Bug report","body":"Body text","state":"open","labels":[],"assignees":[{"login":"alice"}],"milestone":null}"#,
        ),
        ScriptedResponse::json(
            "PATCH /gitbucket/api/v3/repos/alice/project/issues/7 HTTP/1.1",
            "404 Not Found",
            r#"{"message":"Not Found"}"#,
        ),
        ScriptedResponse::html("POST /gitbucket/signin HTTP/1.1", "200 OK", "signed in")
            .with_header("set-cookie", "JSESSIONID=session347; Path=/; HttpOnly"),
        ScriptedResponse::html(
            "POST /gitbucket/alice/project/issues/7/assignee/delete HTTP/1.1",
            "200 OK",
            "removed",
        ),
        ScriptedResponse::html(
            "POST /gitbucket/alice/project/issues/7/assignee/new HTTP/1.1",
            "200 OK",
            "added",
        ),
        ScriptedResponse::json(
            "GET /gitbucket/api/v3/repos/alice/project/issues/7 HTTP/1.1",
            "200 OK",
            r#"{"number":7,"title":"Bug report","body":"Body text","state":"open","labels":[],"assignees":[{"login":"bob"}],"milestone":null}"#,
        ),
    ]);

    let output = env
        .repo_api_command(format!("127.0.0.1:{port}/gitbucket"), "alice/project")
        .env("GB_USER", "alice")
        .env("GB_PASSWORD", "secret-pass")
        .args([
            "issue",
            "edit",
            "7",
            "--add-assignee",
            "bob",
            "--remove-assignee",
            "alice",
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
    assert!(requests[3].body.contains("assigneeUserName=alice"));
    assert!(requests[4].body.contains("assigneeUserName=bob"));
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("Updated issue #7: Bug report"),
        "stdout: {}",
        String::from_utf8_lossy(&output.stdout)
    );
}

#[test]
fn issue_label_only_edits_use_dedicated_api_without_issue_patch_or_password() {
    let env = GbTestEnv::new();
    let mut after = initial_issue();
    after["labels"] = json!([{"name":"bug"},{"name":"urgent"}]);
    let (port, server) = spawn_scripted_server(vec![
        item("GET", "/bucket", &initial_issue()),
        repo_label_at("/bucket", "urgent"),
        labels_response("GET", "/bucket", "", &json!([{"name":"bug"}])),
        labels_response("POST", "/bucket", "", &json!([{"name":"urgent"}])),
        labels_response("GET", "/bucket", "", &after["labels"]),
        item("GET", "/bucket", &after),
    ]);
    let output = env
        .repo_api_command(format!("127.0.0.1:{port}/bucket"), "alice/project")
        .env_remove("GB_PASSWORD")
        .args(["issue", "edit", "7", "--add-label", "urgent"])
        .output()
        .unwrap();
    assert_success(&output);
    let requests = server.join().unwrap();
    assert!(requests
        .iter()
        .all(|request| request.method != "PATCH" && !request.target.contains("signin")));
    assert_eq!(
        serde_json::from_str::<Value>(&requests[3].body).unwrap(),
        json!({"labels":["urgent"]})
    );
}

#[test]
fn issue_edit_requires_an_explicit_change() {
    let env = GbTestEnv::new();

    let output = env
        .repo_api_command("127.0.0.1:9", "alice/project")
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
    let env = GbTestEnv::new();

    let output = env
        .repo_api_command("127.0.0.1:9", "alice/project")
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
    let env = GbTestEnv::new();

    let output = env
        .repo_api_command("127.0.0.1:9", "alice/project")
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

fn initial_issue() -> Value {
    json!({"number":7,"title":"Bug report","body":"Body text","state":"open","labels":[{"name":"bug"}],"assignees":[{"login":"alice"}],"milestone":null})
}
fn item(method: &str, prefix: &str, value: &Value) -> ScriptedResponse {
    ScriptedResponse::json(
        &format!("{method} {prefix}/api/v3/repos/alice/project/issues/7 HTTP/1.1"),
        "200 OK",
        &value.to_string(),
    )
}
fn labels_response(method: &str, prefix: &str, suffix: &str, value: &Value) -> ScriptedResponse {
    ScriptedResponse::json(
        &format!("{method} {prefix}/api/v3/repos/alice/project/issues/7/labels{suffix} HTTP/1.1"),
        "200 OK",
        &value.to_string(),
    )
}
fn repo_label(name: &str) -> ScriptedResponse {
    repo_label_at("", name)
}
fn repo_label_at(prefix: &str, name: &str) -> ScriptedResponse {
    ScriptedResponse::json(
        &format!("GET {prefix}/api/v3/repos/alice/project/labels/{name} HTTP/1.1"),
        "200 OK",
        &json!({"name":name,"color":"123abc"}).to_string(),
    )
}

#[test]
fn issue_label_missing_addition_fails_before_metadata_or_implicit_label_creation() {
    let env = GbTestEnv::new();
    let (port, server) = spawn_scripted_server(vec![
        item("GET", "", &initial_issue()),
        ScriptedResponse::json(
            "GET /api/v3/repos/alice/project/labels/missing HTTP/1.1",
            "404 Not Found",
            "{}",
        ),
    ]);
    let output = env
        .repo_api_command(format!("127.0.0.1:{port}"), "alice/project")
        .args([
            "issue",
            "edit",
            "7",
            "--title",
            "must not apply",
            "--add-label",
            "missing",
        ])
        .output()
        .unwrap();
    assert_failure(&output);
    assert!(output.stdout.is_empty());
    assert_eq!(server.join().unwrap().len(), 2);
}

#[test]
fn issue_label_noop_and_add_remove_overlap_do_not_replace_the_set() {
    let env = GbTestEnv::new();
    let (port, server) = spawn_scripted_server(vec![
        item("GET", "", &initial_issue()),
        repo_label("bug"),
        labels_response("GET", "", "", &initial_issue()["labels"]),
        item("GET", "", &initial_issue()),
    ]);
    let output = env
        .repo_api_command(format!("127.0.0.1:{port}"), "alice/project")
        .env_remove("GB_PASSWORD")
        .args([
            "issue",
            "edit",
            "7",
            "--add-label",
            "bug,bug",
            "--remove-label",
            "bug,absent",
        ])
        .output()
        .unwrap();
    assert_success(&output);
    assert!(server
        .join()
        .unwrap()
        .iter()
        .all(|request| request.method == "GET"));
}

#[test]
fn issue_label_deletion_encodes_spaces_and_does_not_remove_unrelated_labels() {
    let env = GbTestEnv::new();
    let mut before = initial_issue();
    before["labels"] = json!([{"name":"needs review"},{"name":"keep"}]);
    let mut after = before.clone();
    after["labels"] = json!([{"name":"keep"}]);
    let (port, server) = spawn_scripted_server(vec![
        item("GET", "", &before),
        labels_response("GET", "", "", &before["labels"]),
        labels_response("DELETE", "", "/needs%20review", &json!([])),
        labels_response("GET", "", "", &after["labels"]),
        item("GET", "", &after),
    ]);
    let output = env
        .repo_api_command(format!("127.0.0.1:{port}"), "alice/project")
        .args(["issue", "edit", "7", "--remove-label", "needs review"])
        .output()
        .unwrap();
    assert_success(&output);
    let requests = server.join().unwrap();
    assert!(requests
        .iter()
        .all(|request| request.method != "PUT" && request.method != "POST"));
}

#[test]
fn issue_later_label_failure_reports_completed_metadata_and_removals() {
    let env = GbTestEnv::new();
    let mut metadata = initial_issue();
    metadata["title"] = json!("changed");
    let (port, server) = spawn_scripted_server(vec![
        item("GET", "", &initial_issue()),
        repo_label("urgent"),
        item("PATCH", "", &metadata),
        item("GET", "", &metadata),
        labels_response("GET", "", "", &initial_issue()["labels"]),
        labels_response("DELETE", "", "/bug", &json!([])),
        ScriptedResponse::json(
            "POST /api/v3/repos/alice/project/issues/7/labels HTTP/1.1",
            "403 Forbidden",
            "{}",
        ),
    ]);
    let output = env
        .repo_api_command(format!("127.0.0.1:{port}"), "alice/project")
        .args([
            "--json-errors",
            "issue",
            "edit",
            "7",
            "--title",
            "changed",
            "--remove-label",
            "bug",
            "--add-label",
            "urgent",
        ])
        .output()
        .unwrap();
    assert_failure(&output);
    assert!(output.stdout.is_empty());
    let error: Value = serde_json::from_slice(&output.stderr).unwrap();
    let message = error["error"]["message"].as_str().unwrap();
    assert!(message.contains("Metadata update steps completed"));
    assert!(message.contains("removed"));
    assert!(message.contains("before retrying"));
    server.join().unwrap();
}

#[test]
fn issue_label_false_success_does_not_print_success() {
    let env = GbTestEnv::new();
    let (port, server) = spawn_scripted_server(vec![
        item("GET", "", &initial_issue()),
        repo_label("urgent"),
        labels_response("GET", "", "", &initial_issue()["labels"]),
        labels_response("POST", "", "", &json!([{"name":"urgent"}])),
        labels_response("GET", "", "", &initial_issue()["labels"]),
    ]);
    let output = env
        .repo_api_command(format!("127.0.0.1:{port}"), "alice/project")
        .args(["issue", "edit", "7", "--add-label", "urgent"])
        .output()
        .unwrap();
    assert_failure(&output);
    assert!(output.stdout.is_empty());
    assert!(stderr(&output).contains("do not match"));
    server.join().unwrap();
}

#[test]
fn issue_metadata_false_success_stops_before_label_writes() {
    let env = GbTestEnv::new();
    let (port, server) = spawn_scripted_server(vec![
        item("GET", "", &initial_issue()),
        repo_label("urgent"),
        item("PATCH", "", &initial_issue()),
        item("GET", "", &initial_issue()),
    ]);
    let output = env
        .repo_api_command(format!("127.0.0.1:{port}"), "alice/project")
        .args([
            "issue",
            "edit",
            "7",
            "--title",
            "not saved",
            "--add-label",
            "urgent",
        ])
        .output()
        .unwrap();
    assert_failure(&output);
    assert!(stderr(&output).contains("stored metadata does not match"));
    assert_eq!(server.join().unwrap().len(), 4);
}
