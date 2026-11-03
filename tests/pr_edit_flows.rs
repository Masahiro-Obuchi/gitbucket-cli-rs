mod support;

use serde_json::{json, Value};
use support::gb_cmd::{assert_failure, assert_success, stderr, GbTestEnv};
use support::mock_http::{spawn_scripted_server, ScriptedResponse};

fn response(
    method: &str,
    prefix: &str,
    path: &str,
    status: &str,
    body: &Value,
) -> ScriptedResponse {
    ScriptedResponse::json(
        &format!("{method} {prefix}/api/v3/{path} HTTP/1.1"),
        status,
        &body.to_string(),
    )
}

const PR: &str = "repos/alice/project/pulls/5";
const ISSUE: &str = "repos/alice/project/issues/5";

fn current() -> Value {
    json!({"number":5,"title":"Old title","body":"keep original body","state":"open","merged":false})
}
fn issue(names: &[&str]) -> Value {
    json!({"number":5,"title":"Old title","body":"keep original body","state":"open","pull_request":{},"assignees":names.iter().map(|name|json!({"login":name})).collect::<Vec<_>>()})
}

#[test]
fn pr_edit_content_and_state_use_pr_patch_without_web_credentials() {
    for (args, expected) in [
        (
            vec!["--title", "New title"],
            json!({"title":"New title","body":"keep original body"}),
        ),
        (
            vec!["--body", "new body"],
            json!({"title":"Old title","body":"new body"}),
        ),
        (vec!["--body", ""], json!({"title":"Old title","body":""})),
        (vec!["--state", "closed"], json!({"state":"closed"})),
        (
            vec![
                "--title",
                "New title",
                "--body",
                "new body",
                "--state",
                "closed",
                "--web",
            ],
            json!({"title":"New title","body":"new body","state":"closed"}),
        ),
    ] {
        let env = GbTestEnv::new();
        let original = current();
        let mut updated = original.clone();
        for (key, value) in expected.as_object().unwrap() {
            updated[key] = value.clone();
        }
        let (port, server) = spawn_scripted_server(vec![
            response("GET", "/bucket", PR, "200 OK", &original),
            response("PATCH", "/bucket", PR, "200 OK", &updated),
        ]);
        let mut command = env.repo_api_command(format!("127.0.0.1:{port}/bucket"), "alice/project");
        command
            .env_remove("GB_PASSWORD")
            .args(["pr", "edit", "5"])
            .args(&args);
        let output = command.output().unwrap();
        assert_success(&output);
        let requests = server.join().unwrap();
        assert_eq!(requests.len(), 2);
        assert_eq!(
            serde_json::from_str::<Value>(&requests[1].body).unwrap(),
            expected
        );
        assert!(!stderr(&output).contains("Web UI"));
    }
}

#[test]
fn pr_title_edit_with_absent_body_keeps_absent_body() {
    let env = GbTestEnv::new();
    let original = json!({"number":5,"title":"Old","state":"closed"});
    let updated = json!({"number":5,"title":"New","state":"closed"});
    let (port, server) = spawn_scripted_server(vec![
        response("GET", "", PR, "200 OK", &original),
        response("PATCH", "", PR, "200 OK", &updated),
    ]);
    let output = env
        .repo_api_command(format!("127.0.0.1:{port}"), "alice/project")
        .args(["pr", "edit", "5", "--title", "New"])
        .output()
        .unwrap();
    assert_success(&output);
    let requests = server.join().unwrap();
    assert_eq!(
        serde_json::from_str::<Value>(&requests[1].body).unwrap(),
        json!({"title":"New"})
    );
}

#[test]
fn pr_patch_errors_never_fall_back_to_issue_patch_or_web_even_when_permitted() {
    for status in [
        "404 Not Found",
        "401 Unauthorized",
        "403 Forbidden",
        "500 Internal Server Error",
    ] {
        let env = GbTestEnv::new();
        let (port, server) = spawn_scripted_server(vec![
            response("GET", "", PR, "200 OK", &current()),
            response("PATCH", "", PR, status, &json!({"message":"rejected"})),
        ]);
        let output = env
            .repo_api_command(format!("127.0.0.1:{port}"), "alice/project")
            .env("GB_PASSWORD", "must-not-be-used")
            .args(["pr", "edit", "5", "--title", "New", "--web"])
            .output()
            .unwrap();
        assert_failure(&output);
        assert!(output.stdout.is_empty());
        assert!(stderr(&output).contains("API error"), "{}", stderr(&output));
        assert_eq!(server.join().unwrap().len(), 2);
    }
}

#[test]
fn pr_assignee_changes_require_permission_before_rest_mutation() {
    let env = GbTestEnv::new();
    let (port, server) = spawn_scripted_server(vec![
        response("GET", "", PR, "200 OK", &current()),
        response("GET", "", ISSUE, "200 OK", &issue(&["alice"])),
    ]);
    let output = env
        .repo_api_command(format!("127.0.0.1:{port}"), "alice/project")
        .args(["pr", "edit", "5", "--title", "New", "--add-assignee", "bob"])
        .output()
        .unwrap();
    assert_failure(&output);
    assert!(output.stdout.is_empty());
    assert!(stderr(&output).contains("require --web"));
    assert_eq!(server.join().unwrap().len(), 2);
}

#[test]
fn pr_assignee_authentication_failure_precedes_rest_mutation() {
    let env = GbTestEnv::new();
    let (port, server) = spawn_scripted_server(vec![
        response("GET", "", PR, "200 OK", &current()),
        response("GET", "", ISSUE, "200 OK", &issue(&["alice"])),
        ScriptedResponse::html("POST /signin HTTP/1.1", "401 Unauthorized", "incorrect"),
    ]);
    let output = env
        .repo_api_command(format!("127.0.0.1:{port}"), "alice/project")
        .env("GB_USER", "alice")
        .env("GB_PASSWORD", "dummy-password")
        .args([
            "pr",
            "edit",
            "5",
            "--title",
            "New",
            "--add-assignee",
            "bob",
            "--web",
        ])
        .output()
        .unwrap();
    assert_failure(&output);
    assert!(output.stdout.is_empty());
    assert!(stderr(&output).contains("sign-in failed"));
    assert_eq!(server.join().unwrap().len(), 3);
}

fn signin() -> ScriptedResponse {
    ScriptedResponse::html("POST /bucket/signin HTTP/1.1", "200 OK", "signed in")
        .with_header("set-cookie", "JSESSIONID=pr-session; Path=/; HttpOnly")
}

#[test]
fn pr_mixed_edit_keeps_assignees_out_of_rest_payload_and_verifies_web_changes() {
    let env = GbTestEnv::new();
    let updated = json!({"number":5,"title":"New","body":"new body","state":"closed"});
    let (port, server) = spawn_scripted_server(vec![
        response("GET", "/bucket", PR, "200 OK", &current()),
        response("GET", "/bucket", ISSUE, "200 OK", &issue(&["alice"])),
        signin(),
        response("PATCH", "/bucket", PR, "200 OK", &updated),
        ScriptedResponse::html(
            "POST /bucket/alice/project/issues/5/assignee/delete HTTP/1.1",
            "200 OK",
            "removed",
        ),
        ScriptedResponse::html(
            "POST /bucket/alice/project/issues/5/assignee/new HTTP/1.1",
            "200 OK",
            "added",
        ),
        response("GET", "/bucket", ISSUE, "200 OK", &issue(&["bob"])),
    ]);
    let output = env
        .repo_api_command(format!("127.0.0.1:{port}/bucket"), "alice/project")
        .env("GB_USER", "alice")
        .env("GB_PASSWORD", "dummy-password")
        .args([
            "pr",
            "edit",
            "5",
            "--title",
            "New",
            "--body",
            "new body",
            "--state",
            "closed",
            "--remove-assignee",
            "alice",
            "--add-assignee",
            "bob",
            "--web",
        ])
        .output()
        .unwrap();
    assert_success(&output);
    let requests = server.join().unwrap();
    assert_eq!(
        serde_json::from_str::<Value>(&requests[3].body).unwrap(),
        json!({"title":"New","body":"new body","state":"closed"})
    );
    let form = |body: &str| {
        url::form_urlencoded::parse(body.as_bytes())
            .into_owned()
            .collect::<std::collections::HashMap<_, _>>()
    };
    assert_eq!(form(&requests[4].body)["assigneeUserName"], "alice");
    assert_eq!(form(&requests[5].body)["assigneeUserName"], "bob");
    assert_eq!(
        requests[5].headers.get("cookie").map(String::as_str),
        Some("JSESSIONID=pr-session")
    );
}

#[test]
fn pr_assignee_noop_needs_no_web_flag_or_authentication() {
    let env = GbTestEnv::new();
    let (port, server) = spawn_scripted_server(vec![
        response("GET", "", PR, "200 OK", &current()),
        response("GET", "", ISSUE, "200 OK", &issue(&["alice"])),
    ]);
    let output = env
        .repo_api_command(format!("127.0.0.1:{port}"), "alice/project")
        .args(["pr", "edit", "5", "--add-assignee", "alice"])
        .output()
        .unwrap();
    assert_success(&output);
    assert_eq!(server.join().unwrap().len(), 2);
}

#[test]
fn pr_assignee_only_edits_do_not_send_an_empty_patch() {
    let env = GbTestEnv::new();
    let (port, server) = spawn_scripted_server(vec![
        response("GET", "/bucket", PR, "200 OK", &current()),
        response("GET", "/bucket", ISSUE, "200 OK", &issue(&[])),
        signin(),
        ScriptedResponse::html(
            "POST /bucket/alice/project/issues/5/assignee/new HTTP/1.1",
            "200 OK",
            "added",
        ),
        response("GET", "/bucket", ISSUE, "200 OK", &issue(&["bob"])),
    ]);
    let output = env
        .repo_api_command(format!("127.0.0.1:{port}/bucket"), "alice/project")
        .env("GB_USER", "alice")
        .env("GB_PASSWORD", "dummy-password")
        .args(["pr", "edit", "5", "--add-assignee", "bob", "--web"])
        .output()
        .unwrap();
    assert_success(&output);
    assert_eq!(server.join().unwrap().len(), 5);
}

#[test]
fn pr_later_assignee_failure_reports_completed_rest_fields() {
    for fails_on_post in [true, false] {
        let env = GbTestEnv::new();
        let updated = json!({"number":5,"title":"New","body":"keep original body","state":"open"});
        let mut responses = vec![
            response("GET", "/bucket", PR, "200 OK", &current()),
            response("GET", "/bucket", ISSUE, "200 OK", &issue(&[])),
            signin(),
            response("PATCH", "/bucket", PR, "200 OK", &updated),
            ScriptedResponse::html(
                "POST /bucket/alice/project/issues/5/assignee/new HTTP/1.1",
                if fails_on_post {
                    "403 Forbidden"
                } else {
                    "200 OK"
                },
                "not saved",
            ),
        ];
        if !fails_on_post {
            responses.push(response("GET", "/bucket", ISSUE, "200 OK", &issue(&[])));
        }
        let (port, server) = spawn_scripted_server(responses);
        let mut command = env.repo_api_command(format!("127.0.0.1:{port}/bucket"), "alice/project");
        if !fails_on_post {
            command.arg("--json-errors");
        }
        let output = command
            .env("GB_USER", "alice")
            .env("GB_PASSWORD", "dummy-password")
            .args([
                "pr",
                "edit",
                "5",
                "--title",
                "New",
                "--add-assignee",
                "bob",
                "--web",
            ])
            .output()
            .unwrap();
        assert_failure(&output);
        assert!(output.stdout.is_empty());
        assert!(
            stderr(&output).contains("REST title/body/state updates completed"),
            "{}",
            stderr(&output)
        );
        assert!(stderr(&output).contains("before retrying"));
        if !fails_on_post {
            let value: Value = serde_json::from_slice(&output.stderr).unwrap();
            assert_eq!(value["error"]["code"], "error");
            assert!(!stderr(&output).contains("Notice:"));
        }
        server.join().unwrap();
    }
}

#[test]
fn pr_merged_reopen_is_rejected_before_other_edits_or_web_auth() {
    for marker in [
        json!({"merged":true}),
        json!({"merged_at":"2026-04-01T00:00:00Z"}),
    ] {
        let env = GbTestEnv::new();
        let mut original = current();
        original["state"] = json!("closed");
        for (key, value) in marker.as_object().unwrap() {
            original[key] = value.clone();
        }
        let (port, server) =
            spawn_scripted_server(vec![response("GET", "", PR, "200 OK", &original)]);
        let output = env
            .repo_api_command(format!("127.0.0.1:{port}"), "alice/project")
            .args([
                "pr",
                "edit",
                "5",
                "--title",
                "must not change",
                "--state",
                "open",
                "--add-assignee",
                "bob",
                "--web",
            ])
            .output()
            .unwrap();
        assert_failure(&output);
        assert!(stderr(&output).contains("Cannot reopen a merged"));
        assert_eq!(server.join().unwrap().len(), 1);
    }
}

#[test]
fn pr_rest_false_success_is_reported_instead_of_web_retry() {
    let env = GbTestEnv::new();
    let (port, server) = spawn_scripted_server(vec![
        response("GET", "", PR, "200 OK", &current()),
        response("PATCH", "", PR, "200 OK", &current()),
    ]);
    let output = env
        .repo_api_command(format!("127.0.0.1:{port}"), "alice/project")
        .args(["pr", "edit", "5", "--title", "New", "--web"])
        .output()
        .unwrap();
    assert_failure(&output);
    assert!(output.stdout.is_empty());
    assert!(stderr(&output).contains("do not match the intended result"));
    assert_eq!(server.join().unwrap().len(), 2);
}

#[test]
fn pr_errors_are_single_json_objects() {
    let env = GbTestEnv::new();
    let (port, server) = spawn_scripted_server(vec![
        response("GET", "", PR, "200 OK", &current()),
        response(
            "PATCH",
            "",
            PR,
            "403 Forbidden",
            &json!({"message":"denied"}),
        ),
    ]);
    let output = env
        .repo_api_command(format!("127.0.0.1:{port}"), "alice/project")
        .args([
            "--json-errors",
            "pr",
            "edit",
            "5",
            "--body",
            "text",
            "--web",
        ])
        .output()
        .unwrap();
    assert_failure(&output);
    let value: Value = serde_json::from_slice(&output.stderr).unwrap();
    assert_eq!(value["error"]["code"], "api_error");
    assert_eq!(value["error"]["status"], 403);
    server.join().unwrap();
}

#[test]
fn pr_close_reopen_and_same_state_use_the_shared_pr_patch_path() {
    for (action, before, after) in [
        ("close", "open", "closed"),
        ("reopen", "closed", "open"),
        ("close", "closed", "closed"),
        ("reopen", "open", "open"),
    ] {
        let env = GbTestEnv::new();
        let mut original = current();
        original["state"] = json!(before);
        let mut updated = original.clone();
        updated["state"] = json!(after);
        let mut responses = vec![response("GET", "", PR, "200 OK", &original)];
        if before != after {
            responses.push(response("PATCH", "", PR, "200 OK", &updated));
        }
        let (port, server) = spawn_scripted_server(responses);
        let output = env
            .repo_api_command(format!("127.0.0.1:{port}"), "alice/project")
            .env_remove("GB_PASSWORD")
            .args(["pr", action, "5"])
            .output()
            .unwrap();
        assert_success(&output);
        let requests = server.join().unwrap();
        assert_eq!(requests.len(), if before == after { 1 } else { 2 });
        if before != after {
            assert_eq!(
                serde_json::from_str::<Value>(&requests[1].body).unwrap(),
                json!({"state":after})
            );
        }
    }
}

#[test]
fn pr_blank_titles_and_empty_edit_are_rejected_before_requests() {
    for args in [
        vec!["pr", "edit", "5"],
        vec!["pr", "edit", "5", "--title", ""],
        vec!["pr", "edit", "5", "--title", " \n\t"],
    ] {
        let env = GbTestEnv::new();
        let output = env
            .repo_api_command("127.0.0.1:9", "alice/project")
            .args(&args)
            .output()
            .unwrap();
        assert_failure(&output);
        assert!(
            stderr(&output).contains(if args.len() == 3 {
                "No pull request changes"
            } else {
                "title must not be blank"
            }),
            "{}",
            stderr(&output)
        );
    }
}

#[test]
fn pr_metadata_edit_on_merged_pr_retains_closed_state_without_reopening() {
    let env = GbTestEnv::new();
    let mut original = current();
    original["state"] = json!("closed");
    original["merged"] = json!(true);
    let mut updated = original.clone();
    updated["title"] = json!("Edited merged title");
    let (port, server) = spawn_scripted_server(vec![
        response("GET", "", PR, "200 OK", &original),
        response("PATCH", "", PR, "200 OK", &updated),
    ]);
    let output = env
        .repo_api_command(format!("127.0.0.1:{port}"), "alice/project")
        .args(["pr", "edit", "5", "--title", "Edited merged title"])
        .output()
        .unwrap();
    assert_success(&output);
    let requests = server.join().unwrap();
    let body: Value = serde_json::from_str(&requests[1].body).unwrap();
    assert!(body.get("state").is_none());
    assert_eq!(body["body"], original["body"]);
}
