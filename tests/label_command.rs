mod support;

use serde_json::{json, Value};
use support::gb_cmd::GbTestEnv;
use support::gb_cmd::{assert_failure, assert_success, stderr};
use support::mock_http::{
    spawn_scripted_server, spawn_server, with_verified_web_session, without_identity_probes,
    ScriptedResponse,
};

#[test]
fn label_list_prints_json() {
    let env = GbTestEnv::new();
    let (port, server) = spawn_server(
        "200 OK",
        r#"[{"name":"bug","color":"fc2929","description":"Broken behavior"}]"#,
    );

    let output = env
        .repo_api_command(format!("127.0.0.1:{port}"), "alice/project")
        .args(["label", "list", "--json"])
        .output()
        .unwrap();

    let request = server.join().unwrap();

    assert!(output.status.success());
    assert_eq!(request.method, "GET");
    assert_eq!(request.target, "/api/v3/repos/alice/project/labels");

    let payload: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(payload[0]["name"], "bug");
    assert_eq!(payload[0]["description"], "Broken behavior");
}

#[test]
fn label_view_prints_details() {
    let env = GbTestEnv::new();
    let (port, server) = spawn_server(
        "200 OK",
        r#"{"name":"bug","color":"fc2929","description":"Broken behavior","url":"http://example.test/api/v3/repos/alice/project/labels/bug"}"#,
    );

    let output = env
        .repo_api_command(format!("127.0.0.1:{port}"), "alice/project")
        .args(["label", "view", "bug"])
        .output()
        .unwrap();

    let request = server.join().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(output.status.success());
    assert_eq!(request.method, "GET");
    assert_eq!(request.target, "/api/v3/repos/alice/project/labels/bug");
    assert!(stdout.contains("bug"), "stdout: {stdout}");
    assert!(stdout.contains("Color: #fc2929"), "stdout: {stdout}");
    assert!(
        stdout.contains("Description: Broken behavior"),
        "stdout: {stdout}"
    );
}

#[test]
fn label_view_url_encodes_label_name() {
    let env = GbTestEnv::new();
    let (port, server) = spawn_server(
        "200 OK",
        r#"{"name":"needs review","color":"fc2929","description":"Broken behavior"}"#,
    );

    let output = env
        .repo_api_command(format!("127.0.0.1:{port}"), "alice/project")
        .args(["label", "view", "needs review"])
        .output()
        .unwrap();

    let request = server.join().unwrap();

    assert!(output.status.success());
    assert_eq!(request.method, "GET");
    assert_eq!(
        request.target,
        "/api/v3/repos/alice/project/labels/needs%20review"
    );
}

#[test]
fn label_create_rejects_invalid_color_before_api_call() {
    let env = GbTestEnv::new();

    let output = env
        .repo_api_command("127.0.0.1:9", "alice/project")
        .args(["label", "create", "bug", "--color", "zzz"])
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .contains("Invalid label color 'zzz'. Expected a 6-digit hex value like ff0000."),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn label_edit_requires_a_change_before_api_call() {
    let env = GbTestEnv::new();

    let output = env
        .repo_api_command("127.0.0.1:9", "alice/project")
        .args(["label", "edit", "bug"])
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains(
            "No label changes requested. Pass at least one of --name, --color, --description, or --remove-description."
        ),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn label_edit_rejects_invalid_color_before_api_call() {
    let env = GbTestEnv::new();

    let output = env
        .repo_api_command("127.0.0.1:9", "alice/project")
        .args(["label", "edit", "bug", "--color", "zzz"])
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .contains("Invalid label color 'zzz'. Expected a 6-digit hex value like ff0000."),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn label_delete_sends_delete_request_when_yes_is_used() {
    let env = GbTestEnv::new();
    let (port, server) = spawn_server("204 No Content", "");

    let output = env
        .repo_api_command(format!("127.0.0.1:{port}"), "alice/project")
        .args(["label", "delete", "bug", "--yes"])
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
    assert_eq!(request.target, "/api/v3/repos/alice/project/labels/bug");
    assert!(stdout.contains("✓ Deleted label bug"), "stdout: {stdout}");
}

fn label(name: &str, color: &str) -> Value {
    json!({"name":name,"color":color})
}
fn response(method: &str, path: &str, status: &str, body: &Value) -> ScriptedResponse {
    ScriptedResponse::json(
        &format!("{method} /bucket/api/v3/repos/alice/project/labels{path} HTTP/1.1"),
        status,
        &body.to_string(),
    )
}
fn page(name: &str, id: u64) -> String {
    let query = url::form_urlencoded::Serializer::new(String::new())
        .append_pair("labels", name)
        .finish();
    format!("<table><tr id='label-row-{id}'><td><a id='label-row-content-{id}' href='/bucket/alice/project/issues?{query}'>label</a></td></tr></table>")
}
fn edit_form(name: &str, color: &str, id: u64) -> String {
    let name = name.replace('&', "&amp;").replace('"', "&quot;");
    format!("<div id='edit-label-area-{id}'><input id='labelName-{id}' value=\"{name}\"><input id='labelColor-{id}' value='#{color}'></div>")
}
fn signin() -> ScriptedResponse {
    ScriptedResponse::html("POST /bucket/signin HTTP/1.1", "200 OK", "signed in")
        .with_header("set-cookie", "JSESSIONID=labels; Path=/; HttpOnly")
}

#[test]
fn label_create_verifies_persisted_values_and_sends_only_supported_fields() {
    let env = GbTestEnv::new();
    let value = label("needs-review", "abcdef");
    let (port, server) = spawn_scripted_server(vec![
        response("POST", "", "201 Created", &value),
        response("GET", "/needs-review", "200 OK", &value),
    ]);
    let output = env
        .repo_api_command(format!("127.0.0.1:{port}/bucket"), "alice/project")
        .args(["label", "create", "needs-review", "--color", "#ABCDEF"])
        .output()
        .unwrap();
    assert_success(&output);
    let requests = server.join().unwrap();
    assert_eq!(
        serde_json::from_str::<Value>(&requests[0].body).unwrap(),
        json!({"name":"needs-review","color":"abcdef"})
    );
}

#[test]
fn label_rename_supplies_stored_color_without_web_or_recreation() {
    let env = GbTestEnv::new();
    let (port, server) = spawn_scripted_server(vec![
        response(
            "GET",
            "/needs%20review",
            "200 OK",
            &label("needs review", "123abc"),
        ),
        response(
            "PATCH",
            "/needs%20review",
            "200 OK",
            &label("reviewed", "123abc"),
        ),
        response("GET", "/reviewed", "200 OK", &label("reviewed", "123abc")),
    ]);
    let output = env
        .repo_api_command(format!("127.0.0.1:{port}/bucket"), "alice/project")
        .env_remove("GB_PASSWORD")
        .args(["label", "edit", "needs review", "--name", "reviewed"])
        .output()
        .unwrap();
    assert_success(&output);
    let requests = server.join().unwrap();
    assert_eq!(
        serde_json::from_str::<Value>(&requests[1].body).unwrap(),
        json!({"name":"reviewed","color":"123abc"})
    );
    assert!(requests
        .iter()
        .all(|request| request.method == "GET" || request.method == "PATCH"));
}

#[test]
fn label_same_name_color_supported_by_rest_needs_no_web_even_if_permitted() {
    let env = GbTestEnv::new();
    let (port, server) = spawn_scripted_server(vec![
        response("GET", "/bug", "200 OK", &label("bug", "123abc")),
        response("PATCH", "/bug", "200 OK", &label("bug", "ffffff")),
        response("GET", "/bug", "200 OK", &label("bug", "ffffff")),
    ]);
    let output = env
        .repo_api_command(format!("127.0.0.1:{port}/bucket"), "alice/project")
        .env_remove("GB_PASSWORD")
        .args(["label", "edit", "bug", "--color", "ffffff", "--web"])
        .output()
        .unwrap();
    assert_success(&output);
    assert_eq!(server.join().unwrap().len(), 3);
}

#[test]
fn label_same_name_rejection_requires_explicit_web_permission() {
    let env = GbTestEnv::new();
    let (port, server) = spawn_scripted_server(vec![
        response("GET", "/bug", "200 OK", &label("bug", "123abc")),
        response(
            "PATCH",
            "/bug",
            "422 Unprocessable Entity",
            &json!({"message":"Validation Failed"}),
        ),
    ]);
    let output = env
        .repo_api_command(format!("127.0.0.1:{port}/bucket"), "alice/project")
        .args(["label", "edit", "bug", "--color", "ffffff"])
        .output()
        .unwrap();
    assert_failure(&output);
    assert!(output.stdout.is_empty());
    assert!(stderr(&output).contains("Re-run with --web"));
    assert_eq!(server.join().unwrap().len(), 2);
}

#[test]
fn label_color_web_updates_the_same_numeric_id_and_verifies_saved_color() {
    let env = GbTestEnv::new();
    let (port, server) = spawn_scripted_server(with_verified_web_session(vec![
        response(
            "GET",
            "/needs%20review",
            "200 OK",
            &label("needs review", "123abc"),
        ),
        response(
            "PATCH",
            "/needs%20review",
            "422 Unprocessable Entity",
            &json!({}),
        ),
        signin(),
        ScriptedResponse::html(
            "GET /bucket/alice/project/issues/labels HTTP/1.1",
            "200 OK",
            &page("needs review", 42),
        ),
        ScriptedResponse::html(
            "GET /bucket/alice/project/issues/labels/42/edit HTTP/1.1",
            "200 OK",
            &edit_form("needs review", "123abc", 42),
        ),
        ScriptedResponse::html(
            "POST /bucket/alice/project/issues/labels/42/edit HTTP/1.1",
            "200 OK",
            "updated",
        ),
        ScriptedResponse::html(
            "GET /bucket/alice/project/issues/labels HTTP/1.1",
            "200 OK",
            &page("needs review", 42),
        ),
        response(
            "GET",
            "/needs%20review",
            "200 OK",
            &label("needs review", "ffffff"),
        ),
    ]));
    let output = env
        .repo_api_command(format!("127.0.0.1:{port}/bucket"), "alice/project")
        .env("GB_USER", "alice")
        .env("GB_PASSWORD", "dummy-password")
        .args([
            "label",
            "edit",
            "needs review",
            "--color",
            "ffffff",
            "--web",
        ])
        .output()
        .unwrap();
    assert_success(&output);
    let requests = without_identity_probes(server.join().unwrap());
    let form: std::collections::HashMap<_, _> =
        url::form_urlencoded::parse(requests[5].body.as_bytes())
            .into_owned()
            .collect();
    assert_eq!(form["labelName"], "needs review");
    assert_eq!(form["labelColor"], "#ffffff");
    assert_eq!(
        requests[5]
            .headers
            .get("x-requested-with")
            .map(String::as_str),
        Some("XMLHttpRequest")
    );
    assert!(requests.iter().all(|request| request.method != "DELETE"));
}

#[test]
fn label_web_layout_or_snapshot_mismatch_refuses_before_form_submission() {
    for layout in [
        "unknown",
        "duplicate",
        "other-origin",
        "changed-name",
        "changed-color",
    ] {
        let env = GbTestEnv::new();
        let body = match layout {
            "unknown" => "<html>unsupported page</html>".into(),
            "duplicate" => format!("{}{}", page("bug", 42), page("bug", 43)),
            "other-origin" => {
                page("bug", 42).replace("/bucket/alice", "https://other.example/bucket/alice")
            }
            _ => page("bug", 42),
        };
        let mut responses = vec![
            response("GET", "/bug", "200 OK", &label("bug", "123abc")),
            response("PATCH", "/bug", "422 Unprocessable Entity", &json!({})),
            signin(),
            ScriptedResponse::html(
                "GET /bucket/alice/project/issues/labels HTTP/1.1",
                "200 OK",
                &body,
            ),
        ];
        if layout.starts_with("changed-") {
            responses.push(ScriptedResponse::html(
                "GET /bucket/alice/project/issues/labels/42/edit HTTP/1.1",
                "200 OK",
                &edit_form(
                    if layout == "changed-name" {
                        "another-label"
                    } else {
                        "bug"
                    },
                    if layout == "changed-color" {
                        "000000"
                    } else {
                        "123abc"
                    },
                    42,
                ),
            ));
        }
        let (port, server) = spawn_scripted_server(with_verified_web_session(responses));
        let output = env
            .repo_api_command(format!("127.0.0.1:{port}/bucket"), "alice/project")
            .env("GB_USER", "alice")
            .env("GB_PASSWORD", "dummy-password")
            .args(["label", "edit", "bug", "--color", "ffffff", "--web"])
            .output()
            .unwrap();
        assert_failure(&output);
        assert!(output.stdout.is_empty());
        let requests = without_identity_probes(server.join().unwrap());
        assert!(requests
            .iter()
            .filter(|request| request.method == "POST")
            .all(|request| request.target.ends_with("/signin")));
    }
}

#[test]
fn label_description_requests_are_rejected_before_any_mutation_or_prompt() {
    for args in [
        vec![
            "label",
            "create",
            "bug",
            "--color",
            "123abc",
            "--description",
            "ignored",
        ],
        vec!["label", "edit", "bug", "--description", "new"],
        vec!["label", "edit", "bug", "--remove-description"],
        vec![
            "label",
            "edit",
            "bug",
            "--name",
            "renamed",
            "--description",
            "",
        ],
    ] {
        let env = GbTestEnv::new();
        let output = env
            .repo_api_command("127.0.0.1:9", "alice/project")
            .args(&args)
            .stdin(std::process::Stdio::null())
            .output()
            .unwrap();
        assert_failure(&output);
        assert!(
            stderr(&output).contains("description writes are unsupported"),
            "{}",
            stderr(&output)
        );
    }
}

#[test]
fn label_unknown_target_conflicting_rename_and_rest_errors_never_trigger_web() {
    for status in [
        "404 Not Found",
        "403 Forbidden",
        "500 Internal Server Error",
        "422 Unprocessable Entity",
    ] {
        let env = GbTestEnv::new();
        let (port, server) = spawn_scripted_server(vec![
            response("GET", "/bug", "200 OK", &label("bug", "123abc")),
            response("PATCH", "/bug", status, &json!({})),
        ]);
        let output = env
            .repo_api_command(format!("127.0.0.1:{port}/bucket"), "alice/project")
            .env("GB_PASSWORD", "must-not-be-used")
            .args(["label", "edit", "bug", "--name", "another", "--web"])
            .output()
            .unwrap();
        assert_failure(&output);
        assert!(stderr(&output).contains("API error"));
        assert_eq!(server.join().unwrap().len(), 2);
    }
    let env = GbTestEnv::new();
    let (port, server) = spawn_scripted_server(vec![response(
        "GET",
        "/missing",
        "404 Not Found",
        &json!({}),
    )]);
    let output = env
        .repo_api_command(format!("127.0.0.1:{port}/bucket"), "alice/project")
        .args(["label", "edit", "missing", "--color", "ffffff", "--web"])
        .output()
        .unwrap();
    assert_failure(&output);
    assert_eq!(server.join().unwrap().len(), 1);
}

#[test]
fn label_noop_does_not_patch_or_signin() {
    let env = GbTestEnv::new();
    let (port, server) = spawn_scripted_server(vec![response(
        "GET",
        "/bug",
        "200 OK",
        &label("bug", "123abc"),
    )]);
    let output = env
        .repo_api_command(format!("127.0.0.1:{port}/bucket"), "alice/project")
        .args([
            "label", "edit", "bug", "--name", "bug", "--color", "#123ABC",
        ])
        .output()
        .unwrap();
    assert_success(&output);
    assert_eq!(server.join().unwrap().len(), 1);
}

#[test]
fn label_false_success_or_changed_numeric_id_does_not_print_success() {
    let env = GbTestEnv::new();
    let (port, server) = spawn_scripted_server(vec![
        response("GET", "/bug", "200 OK", &label("bug", "123abc")),
        response("PATCH", "/bug", "200 OK", &label("renamed", "123abc")),
        response("GET", "/renamed", "200 OK", &label("renamed", "000000")),
    ]);
    let output = env
        .repo_api_command(format!("127.0.0.1:{port}/bucket"), "alice/project")
        .args(["label", "edit", "bug", "--name", "renamed"])
        .output()
        .unwrap();
    assert_failure(&output);
    assert!(output.stdout.is_empty());
    assert!(stderr(&output).contains("does not match"));
    server.join().unwrap();
}

#[test]
fn label_dot_segments_never_reach_any_api_endpoint() {
    for args in [
        vec!["label", "view", ".."],
        vec!["label", "delete", "..", "--yes"],
        vec!["label", "edit", ".", "--color", "123abc"],
        vec!["label", "create", "..", "--color", "123abc"],
    ] {
        let env = GbTestEnv::new();
        let output = env
            .repo_api_command("127.0.0.1:9", "alice/project")
            .args(&args)
            .output()
            .unwrap();
        assert_failure(&output);
        assert!(
            stderr(&output).contains("dot segments"),
            "{}",
            stderr(&output)
        );
    }
}

#[test]
fn label_web_post_is_not_success_until_identity_and_color_are_verified() {
    for mode in ["id-changed", "not-saved", "verification-failed"] {
        let env = GbTestEnv::new();
        let mut responses = vec![
            response("GET", "/bug", "200 OK", &label("bug", "123abc")),
            response("PATCH", "/bug", "422 Unprocessable Entity", &json!({})),
            signin(),
            ScriptedResponse::html(
                "GET /bucket/alice/project/issues/labels HTTP/1.1",
                "200 OK",
                &page("bug", 42),
            ),
            ScriptedResponse::html(
                "GET /bucket/alice/project/issues/labels/42/edit HTTP/1.1",
                "200 OK",
                &edit_form("bug", "123abc", 42),
            ),
            ScriptedResponse::html(
                "POST /bucket/alice/project/issues/labels/42/edit HTTP/1.1",
                "200 OK",
                "submitted",
            ),
            ScriptedResponse::html(
                "GET /bucket/alice/project/issues/labels HTTP/1.1",
                if mode == "verification-failed" {
                    "500 Internal Server Error"
                } else {
                    "200 OK"
                },
                &page("bug", if mode == "id-changed" { 43 } else { 42 }),
            ),
        ];
        if mode == "not-saved" {
            responses.push(response("GET", "/bug", "200 OK", &label("bug", "123abc")));
        }
        let (port, server) = spawn_scripted_server(with_verified_web_session(responses));
        let output = env
            .repo_api_command(format!("127.0.0.1:{port}/bucket"), "alice/project")
            .env("GB_USER", "alice")
            .env("GB_PASSWORD", "dummy-password")
            .args(["label", "edit", "bug", "--color", "ffffff", "--web"])
            .output()
            .unwrap();
        assert_failure(&output);
        assert!(output.stdout.is_empty());
        assert!(
            stderr(&output).contains("before retrying"),
            "{}",
            stderr(&output)
        );
        assert!(server
            .join()
            .unwrap()
            .iter()
            .all(|request| request.method != "DELETE"));
    }
}

#[test]
fn label_lossy_web_form_values_are_rejected_before_authentication() {
    for name in [" bug ", "bug,urgent", "has-description"] {
        let env = GbTestEnv::new();
        let mut original = label(name, "123abc");
        if name == "has-description" {
            original["description"] = json!("keep existing description");
        }
        let path = if name == " bug " {
            "/%20bug%20"
        } else if name == "bug,urgent" {
            "/bug,urgent"
        } else {
            "/has-description"
        };
        let (port, server) = spawn_scripted_server(vec![
            response("GET", path, "200 OK", &original),
            response("PATCH", path, "422 Unprocessable Entity", &json!({})),
        ]);
        let output = env
            .repo_api_command(format!("127.0.0.1:{port}/bucket"), "alice/project")
            .env_remove("GB_PASSWORD")
            .args(["label", "edit", name, "--color", "ffffff", "--web"])
            .output()
            .unwrap();
        assert_failure(&output);
        assert!(
            stderr(&output).contains("cannot preserve this name or description"),
            "{}",
            stderr(&output)
        );
        assert_eq!(server.join().unwrap().len(), 2);
    }
}
