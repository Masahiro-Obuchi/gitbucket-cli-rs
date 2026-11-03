mod support;

use serde_json::{json, Value};
use support::gb_cmd::GbTestEnv;
use support::gb_cmd::{assert_failure, assert_success, stderr};
use support::mock_http::{spawn_scripted_server, spawn_server, ScriptedResponse};

#[test]
fn milestone_list_prints_json_and_state_query() {
    let env = GbTestEnv::new();
    let (port, server) = spawn_server(
        "200 OK",
        r#"[{"number":7,"title":"v1.0","state":"open","description":"First release","open_issues":3,"closed_issues":1,"due_on":"2026-04-01T00:00:00Z"}]"#,
    );

    let output = env
        .repo_api_command(format!("127.0.0.1:{port}"), "alice/project")
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
    let env = GbTestEnv::new();
    let (port, server) = spawn_server(
        "200 OK",
        r#"{"number":7,"title":"v1.0","state":"open","description":"First release","open_issues":3,"closed_issues":1,"due_on":"2026-04-01T00:00:00Z","html_url":"http://example.test/alice/project/milestone/7"}"#,
    );

    let output = env
        .repo_api_command(format!("127.0.0.1:{port}"), "alice/project")
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
    let env = GbTestEnv::new();
    let (port, server) = spawn_server(
        "200 OK",
        r#"{"number":7,"title":"v1.0","state":"open","description":"First release","open_issues":3,"closed_issues":1,"due_on":"0001-01-01T00:00:00Z"}"#,
    );

    let output = env
        .repo_api_command(format!("127.0.0.1:{port}"), "alice/project")
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
    let env = GbTestEnv::new();
    let (port, server) = spawn_server(
        "201 Created",
        r#"{"number":7,"title":"v1.0","state":"open","description":"First release","due_on":"2026-04-01T00:00:00Z"}"#,
    );

    let output = env
        .repo_api_command(format!("127.0.0.1:{port}"), "alice/project")
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
    let env = GbTestEnv::new();

    let output = env
        .repo_api_command("127.0.0.1:9", "alice/project")
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
fn milestone_edit_requires_an_explicit_change() {
    let env = GbTestEnv::new();

    let output = env
        .repo_api_command("127.0.0.1:9", "alice/project")
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
    let env = GbTestEnv::new();
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

    let output = env
        .repo_api_command(format!("127.0.0.1:{port}"), "alice/project")
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
fn milestone_delete_sends_delete_request_when_yes_is_used() {
    let env = GbTestEnv::new();
    let (port, server) = spawn_server("204 No Content", "");

    let output = env
        .repo_api_command(format!("127.0.0.1:{port}"), "alice/project")
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

fn stored_milestone() -> Value {
    json!({"number":7,"title":"v1.0","state":"closed","description":" preserve description\nwith details ","due_on":"2026-04-01T09:30:00.123+09:00"})
}

fn response(
    method: &str,
    prefix: &str,
    number: bool,
    status: &str,
    value: &Value,
) -> ScriptedResponse {
    ScriptedResponse::json(
        &format!(
            "{method} {prefix}/api/v3/repos/alice/project/milestones{} HTTP/1.1",
            if number { "/7" } else { "" }
        ),
        status,
        &value.to_string(),
    )
}

fn form(fields: &str) -> std::collections::HashMap<String, String> {
    url::form_urlencoded::parse(fields.as_bytes())
        .into_owned()
        .collect()
}

#[test]
fn milestone_single_field_edits_send_all_other_values_unchanged() {
    for (flag, value, key, expected_value) in [
        ("--title", "v1.1", "title", json!("v1.1")),
        ("--description", "Updated", "description", json!("Updated")),
        ("--state", "open", "state", json!("open")),
        (
            "--due-on",
            "2026-05-02",
            "due_on",
            json!("2026-05-02T00:00:00Z"),
        ),
        ("--due-on", "", "due_on", Value::Null),
        ("--description", "", "description", Value::Null),
    ] {
        let env = GbTestEnv::new();
        let original = stored_milestone();
        let mut expected = original.clone();
        expected[key] = expected_value;
        let (port, server) = spawn_scripted_server(vec![
            response("GET", "", true, "200 OK", &original),
            response("PATCH", "", true, "200 OK", &expected),
        ]);
        let output = env
            .repo_api_command(format!("127.0.0.1:{port}"), "alice/project")
            .env_remove("GB_PASSWORD")
            .args(["milestone", "edit", "7", flag, value])
            .output()
            .unwrap();
        let requests = server.join().unwrap();
        assert_success(&output);
        let mut body: Value = serde_json::from_str(&requests[1].body).unwrap();
        // The request is a complete replacement, with explicit null for clearing.
        body["number"] = json!(7);
        assert_eq!(body, expected, "wrong replacement for {flag} {value:?}");
        assert!(!stderr(&output).contains("fallback"));
    }
}

#[test]
fn milestone_updates_preserve_null_fields_and_close_state() {
    let env = GbTestEnv::new();
    let original =
        json!({"number":7,"title":"v1.0","state":"closed","description":null,"due_on":null});
    let (port, server) = spawn_scripted_server(vec![
        response("GET", "", true, "200 OK", &original),
        response("PATCH", "", true, "200 OK", &original),
    ]);
    let output = env
        .repo_api_command(format!("127.0.0.1:{port}"), "alice/project")
        .args(["milestone", "edit", "7", "--title", "v1.1"])
        .output()
        .unwrap();
    assert_success(&output);
    let requests = server.join().unwrap();
    let body: Value = serde_json::from_str(&requests[1].body).unwrap();
    assert_eq!(
        body,
        json!({"title":"v1.1","state":"closed","description":null,"due_on":null})
    );
}

#[test]
fn milestone_rest_create_and_delete_errors_never_start_web_signin() {
    for action in ["create", "delete"] {
        for status in [
            "404 Not Found",
            "401 Unauthorized",
            "403 Forbidden",
            "500 Internal Server Error",
        ] {
            let env = GbTestEnv::new();
            let method = if action == "create" { "POST" } else { "DELETE" };
            let (port, server) = spawn_scripted_server(vec![response(
                method,
                "/bucket",
                action == "delete",
                status,
                &json!({"message":"failure"}),
            )]);
            let mut command =
                env.repo_api_command(format!("127.0.0.1:{port}/bucket"), "alice/project");
            command
                .env("GB_USER", "alice")
                .env("GB_PASSWORD", "must-not-be-used");
            let args = if action == "create" {
                vec!["milestone", "create", "v1.0"]
            } else {
                vec!["milestone", "delete", "7", "--yes"]
            };
            let output = command.args(args).output().unwrap();
            assert_failure(&output);
            assert!(output.stdout.is_empty());
            assert!(stderr(&output).contains("API error"), "{}", stderr(&output));
            assert_eq!(server.join().unwrap().len(), 1);
        }
    }
}

#[test]
fn milestone_patch_errors_do_not_mean_missing_rest_support() {
    for status in [
        "404 Not Found",
        "401 Unauthorized",
        "403 Forbidden",
        "500 Internal Server Error",
    ] {
        let env = GbTestEnv::new();
        let (port, server) = spawn_scripted_server(vec![
            response("GET", "", true, "200 OK", &stored_milestone()),
            response(
                "PATCH",
                "",
                true,
                status,
                &json!({"message":"failed update"}),
            ),
        ]);
        let output = env
            .repo_api_command(format!("127.0.0.1:{port}"), "alice/project")
            .env("GB_PASSWORD", "must-not-be-used")
            .args(["milestone", "edit", "7", "--title", "v1.1"])
            .output()
            .unwrap();
        assert_failure(&output);
        assert!(output.stdout.is_empty());
        assert!(stderr(&output).contains("API error"), "{}", stderr(&output));
        assert_eq!(server.join().unwrap().len(), 2);
    }
}

#[test]
fn milestone_missing_edit_target_never_attempts_a_mutation() {
    let env = GbTestEnv::new();
    let (port, server) = spawn_scripted_server(vec![response(
        "GET",
        "",
        true,
        "404 Not Found",
        &json!({"message":"missing"}),
    )]);
    let output = env
        .repo_api_command(format!("127.0.0.1:{port}"), "alice/project")
        .args(["milestone", "edit", "7", "--title", "日本語 release"])
        .output()
        .unwrap();
    assert_failure(&output);
    assert!(stderr(&output).contains("API error (404)"));
    assert_eq!(server.join().unwrap().len(), 1);
}

#[test]
fn milestone_invalid_titles_fail_before_api_calls() {
    for title in [String::new(), " ".into(), "x".repeat(101), "🦀".repeat(51)] {
        for action in ["create", "edit"] {
            let env = GbTestEnv::new();
            let mut command = env.repo_api_command("127.0.0.1:9", "alice/project");
            if action == "create" {
                command.args(["milestone", "create", &title]);
            } else {
                command.args(["milestone", "edit", "7", "--title", &title]);
            }
            let output = command.output().unwrap();
            assert_failure(&output);
            assert!(
                stderr(&output).contains("Milestone title must be"),
                "{}",
                stderr(&output)
            );
        }
    }
}

fn signin() -> ScriptedResponse {
    ScriptedResponse::html("POST /bucket/signin HTTP/1.1", "200 OK", "signed in")
        .with_header("set-cookie", "JSESSIONID=session123; Path=/; HttpOnly")
}

#[test]
fn milestone_unicode_create_uses_web_only_after_known_title_rejection() {
    let env = GbTestEnv::new();
    let milestone = json!({"number":7,"title":"日本語 release","state":"open","description":"説明","due_on":"2026-04-01T00:00:00Z"});
    let (port, server) = spawn_scripted_server(vec![
        response("POST", "/bucket", false, "404 Not Found", &json!({})),
        ScriptedResponse::json(
            "GET /bucket/api/v3/repos/alice/project/milestones?state=all HTTP/1.1",
            "200 OK",
            "[]",
        ),
        signin(),
        ScriptedResponse::html(
            "POST /bucket/alice/project/issues/milestones/new HTTP/1.1",
            "200 OK",
            "created",
        ),
        ScriptedResponse::json(
            "GET /bucket/api/v3/repos/alice/project/milestones?state=all HTTP/1.1",
            "200 OK",
            &json!([milestone]).to_string(),
        ),
    ]);
    let output = env
        .repo_api_command(format!("127.0.0.1:{port}/bucket"), "alice/project")
        .env("GB_USER", "alice")
        .env("GB_PASSWORD", "dummy-password")
        .args([
            "milestone",
            "create",
            "日本語 release",
            "--description",
            "説明",
            "--due-on",
            "2026-04-01",
        ])
        .output()
        .unwrap();
    assert_success(&output);
    let requests = server.join().unwrap();
    assert_eq!(form(&requests[3].body)["title"], "日本語 release");
    assert_eq!(form(&requests[3].body)["description"], "説明");
    assert_eq!(form(&requests[3].body)["dueDate"], "2026-04-01");
    assert_eq!(
        requests[3].headers.get("cookie").map(String::as_str),
        Some("JSESSIONID=session123")
    );
}

#[test]
fn milestone_unicode_title_accepted_by_rest_does_not_require_web() {
    let env = GbTestEnv::new();
    let milestone = json!({"number":7,"title":"日本語 release","state":"open"});
    let (port, server) =
        spawn_scripted_server(vec![response("POST", "", false, "201 Created", &milestone)]);
    let output = env
        .repo_api_command(format!("127.0.0.1:{port}"), "alice/project")
        .env_remove("GB_PASSWORD")
        .args(["milestone", "create", "日本語 release"])
        .output()
        .unwrap();
    assert_success(&output);
    assert_eq!(server.join().unwrap().len(), 1);
}

#[test]
fn milestone_web_edit_keeps_closed_state_and_clears_fields_explicitly() {
    let env = GbTestEnv::new();
    let current = json!({"number":7,"title":"日本語 release","state":"closed","description":"説明","due_on":"2026-04-01T00:00:00Z"});
    let updated = json!({"number":7,"title":"日本語 release","state":"closed","description":"","due_on":null});
    let (port, server) = spawn_scripted_server(vec![
        response("GET", "/bucket", true, "200 OK", &current),
        response("PATCH", "/bucket", true, "404 Not Found", &json!({})),
        response("GET", "/bucket", true, "200 OK", &current),
        signin(),
        ScriptedResponse::html(
            "POST /bucket/alice/project/issues/milestones/7/edit HTTP/1.1",
            "200 OK",
            "updated",
        ),
        response("GET", "/bucket", true, "200 OK", &updated),
    ]);
    let output = env
        .repo_api_command(format!("127.0.0.1:{port}/bucket"), "alice/project")
        .env("GB_USER", "alice")
        .env("GB_PASSWORD", "dummy-password")
        .args([
            "milestone",
            "edit",
            "7",
            "--description",
            "",
            "--due-on",
            "",
        ])
        .output()
        .unwrap();
    assert_success(&output);
    let requests = server.join().unwrap();
    assert_eq!(form(&requests[4].body)["title"], "日本語 release");
    assert_eq!(form(&requests[4].body)["description"], "");
    assert_eq!(form(&requests[4].body)["dueDate"], "");
    let body: Value = serde_json::from_str(&requests[1].body).unwrap();
    assert_eq!(body["state"], "closed");
    assert!(body["due_on"].is_null());
}

#[test]
fn milestone_unicode_state_only_uses_only_web_state_action_and_preserves_timestamp() {
    let env = GbTestEnv::new();
    let mut current = stored_milestone();
    current["title"] = json!("日本語 release");
    let mut updated = current.clone();
    updated["state"] = json!("open");
    let (port, server) = spawn_scripted_server(vec![
        response("GET", "/bucket", true, "200 OK", &current),
        response("PATCH", "/bucket", true, "404 Not Found", &json!({})),
        response("GET", "/bucket", true, "200 OK", &current),
        signin(),
        ScriptedResponse::html(
            "GET /bucket/alice/project/issues/milestones/7/open HTTP/1.1",
            "200 OK",
            "opened",
        ),
        response("GET", "/bucket", true, "200 OK", &updated),
    ]);
    let output = env
        .repo_api_command(format!("127.0.0.1:{port}/bucket"), "alice/project")
        .env("GB_USER", "alice")
        .env("GB_PASSWORD", "dummy-password")
        .args(["milestone", "edit", "7", "--state", "open"])
        .output()
        .unwrap();
    assert_success(&output);
    assert_eq!(server.join().unwrap().len(), 6);
}

#[test]
fn milestone_web_rejects_lossy_unchanged_due_or_whitespace_before_signin() {
    for whitespace in [false, true] {
        let env = GbTestEnv::new();
        let current = json!({"number":7,"title":"日本語 release","state":"closed","description":if whitespace { " keep me " } else { "説明" },"due_on":if whitespace { Value::Null } else { json!("2026-04-01T09:30:00Z") }});
        let (port, server) = spawn_scripted_server(vec![
            response("GET", "/bucket", true, "200 OK", &current),
            response("PATCH", "/bucket", true, "404 Not Found", &json!({})),
            response("GET", "/bucket", true, "200 OK", &current),
        ]);
        let output = env
            .repo_api_command(format!("127.0.0.1:{port}/bucket"), "alice/project")
            .env("GB_PASSWORD", "must-not-be-used")
            .args(["milestone", "edit", "7", "--title", "新しい release"])
            .output()
            .unwrap();
        assert_failure(&output);
        assert!(output.stdout.is_empty());
        assert!(
            stderr(&output).contains(if whitespace {
                "trims title/description"
            } else {
                "cannot guarantee preservation"
            }),
            "{}",
            stderr(&output)
        );
        assert_eq!(server.join().unwrap().len(), 3);
    }
}

#[test]
fn milestone_web_rejects_java_trim_control_characters_before_signin() {
    for create in [true, false] {
        let env = GbTestEnv::new();
        let current = json!({"number":7,"title":"日本語 release","state":"closed","description":"keep this\u{1b}"});
        let responses = if create {
            vec![response(
                "POST",
                "/bucket",
                false,
                "404 Not Found",
                &json!({}),
            )]
        } else {
            vec![
                response("GET", "/bucket", true, "200 OK", &current),
                response("PATCH", "/bucket", true, "404 Not Found", &json!({})),
                response("GET", "/bucket", true, "200 OK", &current),
            ]
        };
        let (port, server) = spawn_scripted_server(responses);
        let mut command = env.repo_api_command(format!("127.0.0.1:{port}/bucket"), "alice/project");
        command
            .env("GB_USER", "alice")
            .env("GB_PASSWORD", "must-not-be-used");
        if create {
            command.args(["milestone", "create", "日本語 release\u{1b}"]);
        } else {
            command.args(["milestone", "edit", "7", "--title", "新しい release"]);
        }
        let output = command.output().unwrap();
        assert_failure(&output);
        assert!(output.stdout.is_empty());
        assert!(
            stderr(&output).contains("trims title/description"),
            "{}",
            stderr(&output)
        );
        assert_eq!(server.join().unwrap().len(), if create { 1 } else { 3 });
    }
}

#[test]
fn milestone_web_preserves_unicode_spacing_that_java_trim_retains() {
    let env = GbTestEnv::new();
    let title = "\u{3000}日本語 release\u{3000}";
    let description = "\u{a0}説明\u{a0}";
    let milestone = json!({"number":7,"title":title,"state":"open","description":description});
    let (port, server) = spawn_scripted_server(vec![
        response("POST", "/bucket", false, "404 Not Found", &json!({})),
        ScriptedResponse::json(
            "GET /bucket/api/v3/repos/alice/project/milestones?state=all HTTP/1.1",
            "200 OK",
            "[]",
        ),
        signin(),
        ScriptedResponse::html(
            "POST /bucket/alice/project/issues/milestones/new HTTP/1.1",
            "200 OK",
            "created",
        ),
        ScriptedResponse::json(
            "GET /bucket/api/v3/repos/alice/project/milestones?state=all HTTP/1.1",
            "200 OK",
            &json!([milestone]).to_string(),
        ),
    ]);
    let output = env
        .repo_api_command(format!("127.0.0.1:{port}/bucket"), "alice/project")
        .env("GB_USER", "alice")
        .env("GB_PASSWORD", "dummy-password")
        .args(["milestone", "create", title, "--description", description])
        .output()
        .unwrap();
    assert_success(&output);
    let requests = server.join().unwrap();
    assert_eq!(form(&requests[3].body)["title"], title);
    assert_eq!(form(&requests[3].body)["description"], description);
}

#[test]
fn milestone_web_false_success_is_not_printed_as_success() {
    let env = GbTestEnv::new();
    let current =
        json!({"number":7,"title":"日本語 release","state":"closed","description":"説明"});
    let (port, server) = spawn_scripted_server(vec![
        response("GET", "/bucket", true, "200 OK", &current),
        response("PATCH", "/bucket", true, "404 Not Found", &json!({})),
        response("GET", "/bucket", true, "200 OK", &current),
        signin(),
        ScriptedResponse::html(
            "POST /bucket/alice/project/issues/milestones/7/edit HTTP/1.1",
            "200 OK",
            "validation error",
        ),
        response("GET", "/bucket", true, "200 OK", &current),
    ]);
    let output = env
        .repo_api_command(format!("127.0.0.1:{port}/bucket"), "alice/project")
        .env("GB_USER", "alice")
        .env("GB_PASSWORD", "dummy-password")
        .args(["milestone", "edit", "7", "--description", "更新"])
        .output()
        .unwrap();
    assert_failure(&output);
    assert!(output.stdout.is_empty());
    assert!(stderr(&output).contains("stored fields do not match"));
    server.join().unwrap();
}

#[test]
fn milestone_unicode_create_missing_repository_does_not_prompt_for_web_auth() {
    let env = GbTestEnv::new();
    let (port, server) = spawn_scripted_server(vec![
        response("POST", "/bucket", false, "404 Not Found", &json!({})),
        ScriptedResponse::json(
            "GET /bucket/api/v3/repos/alice/project/milestones?state=all HTTP/1.1",
            "404 Not Found",
            "{}",
        ),
    ]);
    let output = env
        .repo_api_command(format!("127.0.0.1:{port}/bucket"), "alice/project")
        .args(["milestone", "create", "日本語 release"])
        .output()
        .unwrap();
    assert_failure(&output);
    assert!(stderr(&output).contains("API error (404)"));
    server.join().unwrap();
}

#[test]
fn milestone_web_create_verification_failure_does_not_report_success() {
    for list_fails in [false, true] {
        let env = GbTestEnv::new();
        let (port, server) = spawn_scripted_server(vec![
            response("POST", "/bucket", false, "404 Not Found", &json!({})),
            ScriptedResponse::json(
                "GET /bucket/api/v3/repos/alice/project/milestones?state=all HTTP/1.1",
                "200 OK",
                "[]",
            ),
            signin(),
            ScriptedResponse::html(
                "POST /bucket/alice/project/issues/milestones/new HTTP/1.1",
                "200 OK",
                "submitted",
            ),
            ScriptedResponse::json(
                "GET /bucket/api/v3/repos/alice/project/milestones?state=all HTTP/1.1",
                if list_fails {
                    "401 Unauthorized"
                } else {
                    "200 OK"
                },
                if list_fails { "{}" } else { "[]" },
            ),
        ]);
        let output = env
            .repo_api_command(format!("127.0.0.1:{port}/bucket"), "alice/project")
            .env("GB_USER", "alice")
            .env("GB_PASSWORD", "dummy-password")
            .args(["milestone", "create", "日本語 release"])
            .output()
            .unwrap();
        assert_failure(&output);
        assert!(output.stdout.is_empty());
        assert!(stderr(&output).contains("before retrying"));
        server.join().unwrap();
    }
}

#[test]
fn milestone_web_create_duplicate_title_does_not_attempt_signin_or_mutation() {
    let env = GbTestEnv::new();
    let (port, server) = spawn_scripted_server(vec![
        response("POST", "/bucket", false, "404 Not Found", &json!({})),
        ScriptedResponse::json(
            "GET /bucket/api/v3/repos/alice/project/milestones?state=all HTTP/1.1",
            "200 OK",
            r#"[{"number":7,"title":"日本語 Release","state":"open"}]"#,
        ),
    ]);
    let output = env
        .repo_api_command(format!("127.0.0.1:{port}/bucket"), "alice/project")
        .args(["milestone", "create", "日本語 release"])
        .output()
        .unwrap();
    assert_failure(&output);
    assert!(stderr(&output).contains("already exists"));
    assert_eq!(server.join().unwrap().len(), 2);
}

#[test]
fn milestone_deleted_between_patch_and_web_never_requests_password() {
    let env = GbTestEnv::new();
    let current = json!({"number":7,"title":"日本語 release","state":"closed"});
    let (port, server) = spawn_scripted_server(vec![
        response("GET", "/bucket", true, "200 OK", &current),
        response("PATCH", "/bucket", true, "404 Not Found", &json!({})),
        response("GET", "/bucket", true, "404 Not Found", &json!({})),
    ]);
    let output = env
        .repo_api_command(format!("127.0.0.1:{port}/bucket"), "alice/project")
        .args(["milestone", "edit", "7", "--description", "更新"])
        .output()
        .unwrap();
    assert_failure(&output);
    assert!(stderr(&output).contains("API error (404)"));
    assert_eq!(server.join().unwrap().len(), 3);
}

#[test]
fn milestone_web_edit_reloads_latest_values_and_handles_legacy_unset_due() {
    let env = GbTestEnv::new();
    let current = json!({"number":7,"title":"日本語 release","state":"open","description":"old","due_on":"0001-01-01T00:00:00Z"});
    let mut latest = current.clone();
    latest["description"] = json!("keep latest");
    latest["state"] = json!("closed");
    let mut expected = latest.clone();
    expected["title"] = json!("新しい release");
    expected["due_on"] = Value::Null;
    let (port, server) = spawn_scripted_server(vec![
        response("GET", "/bucket", true, "200 OK", &current),
        response("PATCH", "/bucket", true, "404 Not Found", &json!({})),
        response("GET", "/bucket", true, "200 OK", &latest),
        signin(),
        ScriptedResponse::html(
            "POST /bucket/alice/project/issues/milestones/7/edit HTTP/1.1",
            "200 OK",
            "updated",
        ),
        response("GET", "/bucket", true, "200 OK", &expected),
    ]);
    let output = env
        .repo_api_command(format!("127.0.0.1:{port}/bucket"), "alice/project")
        .env("GB_USER", "alice")
        .env("GB_PASSWORD", "dummy-password")
        .args(["milestone", "edit", "7", "--title", "新しい release"])
        .output()
        .unwrap();
    assert_success(&output);
    let requests = server.join().unwrap();
    assert_eq!(form(&requests[4].body)["description"], "keep latest");
    assert_eq!(form(&requests[4].body)["dueDate"], "");
}

#[test]
fn milestone_repeating_state_needs_no_patch_or_web_credentials_even_for_unicode() {
    let env = GbTestEnv::new();
    let current = json!({"number":7,"title":"日本語 release","state":"closed","description":" keep whitespace ","due_on":"2026-04-01T09:30:00Z"});
    let (port, server) = spawn_scripted_server(vec![response("GET", "", true, "200 OK", &current)]);
    let output = env
        .repo_api_command(format!("127.0.0.1:{port}"), "alice/project")
        .env_remove("GB_PASSWORD")
        .args(["milestone", "edit", "7", "--state", "closed"])
        .output()
        .unwrap();
    assert_success(&output);
    assert_eq!(server.join().unwrap().len(), 1);
}

#[test]
fn milestone_refresh_to_rest_compatible_title_avoids_web_authentication() {
    let env = GbTestEnv::new();
    let current = json!({"number":7,"title":"日本語 release","state":"closed","description":"old"});
    let latest = json!({"number":7,"title":"rest-title","state":"closed","description":"old","due_on":"2026-04-01T09:30:00Z"});
    let mut updated = latest.clone();
    updated["description"] = json!("new");
    let (port, server) = spawn_scripted_server(vec![
        response("GET", "", true, "200 OK", &current),
        response("PATCH", "", true, "404 Not Found", &json!({})),
        response("GET", "", true, "200 OK", &latest),
        response("PATCH", "", true, "200 OK", &updated),
    ]);
    let output = env
        .repo_api_command(format!("127.0.0.1:{port}"), "alice/project")
        .env_remove("GB_PASSWORD")
        .args(["milestone", "edit", "7", "--description", "new"])
        .output()
        .unwrap();
    assert_success(&output);
    let requests = server.join().unwrap();
    let body: Value = serde_json::from_str(&requests[3].body).unwrap();
    assert_eq!(body["title"], "rest-title");
    assert_eq!(body["due_on"], latest["due_on"]);
    assert_eq!(body["state"], "closed");
}

#[test]
fn milestone_later_web_state_failure_reports_possible_applied_content() {
    let env = GbTestEnv::new();
    let current = json!({"number":7,"title":"日本語 release","state":"open","description":"old"});
    let (port, server) = spawn_scripted_server(vec![
        response("GET", "/bucket", true, "200 OK", &current),
        response("PATCH", "/bucket", true, "404 Not Found", &json!({})),
        response("GET", "/bucket", true, "200 OK", &current),
        signin(),
        ScriptedResponse::html(
            "POST /bucket/alice/project/issues/milestones/7/edit HTTP/1.1",
            "200 OK",
            "updated",
        ),
        ScriptedResponse::html(
            "GET /bucket/alice/project/issues/milestones/7/close HTTP/1.1",
            "403 Forbidden",
            "denied",
        ),
    ]);
    let output = env
        .repo_api_command(format!("127.0.0.1:{port}/bucket"), "alice/project")
        .env("GB_USER", "alice")
        .env("GB_PASSWORD", "dummy-password")
        .args([
            "milestone",
            "edit",
            "7",
            "--description",
            "new",
            "--state",
            "closed",
        ])
        .output()
        .unwrap();
    assert_failure(&output);
    assert!(output.stdout.is_empty());
    assert!(
        stderr(&output).contains("Content changes may already be applied"),
        "{}",
        stderr(&output)
    );
    assert!(stderr(&output).contains("before retrying"));
    server.join().unwrap();
}
