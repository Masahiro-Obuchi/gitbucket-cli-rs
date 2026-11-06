mod support;

use serde_json::{json, Value};
use support::gb_cmd::{assert_failure, assert_success, stderr, GbTestEnv};
use support::mock_http::{spawn_scripted_server, ScriptedResponse};

fn api(prefix: &str, method: &str, path: &str, status: &str, value: Value) -> ScriptedResponse {
    ScriptedResponse::json(
        &format!("{method} {prefix}/api/v3/{path} HTTP/1.1"),
        status,
        &value.to_string(),
    )
}
const ISSUE: &str = "repos/alice/project/issues/7";
fn issue(state: &str) -> Value {
    json!({"number":7,"title":"Old","body":"old body","state":state})
}
fn state_preflight(prefix: &str) -> Vec<ScriptedResponse> {
    vec![
        api(prefix, "GET", ISSUE, "200 OK", issue("open")),
        api(prefix, "PATCH", ISSUE, "404 Not Found", json!({})),
    ]
}
fn token_identity(prefix: &str) -> ScriptedResponse {
    api(prefix, "GET", "user", "200 OK", json!({"login":"alice"}))
}
fn signin(prefix: &str) -> ScriptedResponse {
    ScriptedResponse::html(
        &format!("POST {prefix}/signin HTTP/1.1"),
        "200 OK",
        "login response",
    )
    .with_header("set-cookie", "JSESSIONID=session; Path=/; HttpOnly")
}

#[test]
fn login_requires_cookie_principal_even_for_http_200_or_unknown_sso_forms() {
    for prefix in ["", "/bucket"] {
        for (status, value) in [
            ("401 Unauthorized", json!({})),
            ("200 OK", json!({"login":"other"})),
            ("200 OK", json!(null)),
        ] {
            let env = GbTestEnv::new();
            let mut script = state_preflight(prefix);
            script.extend([
                token_identity(prefix),
                signin(prefix),
                api(prefix, "GET", "user", status, value),
            ]);
            let (port, server) = spawn_scripted_server(script);
            let output = env
                .repo_api_command(format!("127.0.0.1:{port}{prefix}"), "alice/project")
                .env("GB_USER", "alice")
                .env("GB_PASSWORD", "must-not-be-printed")
                .args(["--json-errors", "issue", "close", "7"])
                .output()
                .unwrap();
            assert_failure(&output);
            assert!(output.stdout.is_empty());
            let error: Value = serde_json::from_slice(&output.stderr).unwrap();
            assert_eq!(error["error"]["code"], "auth_error");
            assert_eq!(error["error"]["operations"][0]["status"], "not_attempted");
            assert!(!stderr(&output).contains("must-not-be-printed"));
            let requests = server.join().unwrap();
            assert_eq!(
                requests[2].headers.get("authorization").map(String::as_str),
                Some("token test-token")
            );
            assert!(!requests[4].headers.contains_key("authorization"));
            assert_eq!(
                requests[4].headers.get("cookie").map(String::as_str),
                Some("JSESSIONID=session")
            );
        }
    }
}

#[test]
fn signin_form_redisplay_or_interactive_sso_is_diagnosed_before_writes() {
    for redirect in [false, true] {
        let env = GbTestEnv::new();
        let mut script = state_preflight("/bucket");
        script.push(token_identity("/bucket"));
        if redirect {
            script.push(
                ScriptedResponse::html("POST /bucket/signin HTTP/1.1", "302 Found", "")
                    .with_header("location", "http://127.0.0.1:9/sso"),
            );
        } else {
            script.push(ScriptedResponse::html(
                "POST /bucket/signin HTTP/1.1",
                "200 OK",
                "<form><input name='password'></form>",
            ));
            script.push(ScriptedResponse::html(
                "GET /bucket/api/v3/user HTTP/1.1",
                "200 OK",
                "<form>SSO sign in</form>",
            ));
        }
        let (port, server) = spawn_scripted_server(script);
        let output = env
            .repo_api_command(format!("127.0.0.1:{port}/bucket"), "alice/project")
            .env("GB_USER", "alice")
            .env("GB_PASSWORD", "dummy")
            .args(["issue", "close", "7"])
            .output()
            .unwrap();
        assert_failure(&output);
        assert!(stderr(&output).contains("SSO"));
        server.join().unwrap();
    }
}

#[test]
fn different_web_actor_requires_explicit_setting_and_is_in_structured_result() {
    for explicit in [false, true] {
        let env = GbTestEnv::new();
        let mut script = state_preflight("/bucket");
        script.push(token_identity("/bucket"));
        if explicit {
            script.extend([
                signin("/bucket"),
                api("/bucket", "GET", "user", "200 OK", json!({"login":"bob"})),
                ScriptedResponse::html(
                    "POST /bucket/alice/project/issue_comments/state HTTP/1.1",
                    "200 OK",
                    "form",
                ),
                api("/bucket", "GET", ISSUE, "200 OK", issue("open")),
            ]);
        }
        let (port, server) = spawn_scripted_server(script);
        let mut cmd = env.repo_api_command(format!("127.0.0.1:{port}/bucket"), "alice/project");
        cmd.env("GB_USER", "bob").env("GB_PASSWORD", "dummy").args([
            "--json-errors",
            "issue",
            "close",
            "7",
        ]);
        if explicit {
            cmd.env("GB_WEB_USER", "bob");
        }
        let output = cmd.output().unwrap();
        assert_failure(&output);
        let error: Value = serde_json::from_slice(&output.stderr).unwrap();
        assert_eq!(
            error["error"]["operations"][0]["status"],
            if explicit {
                "unverified"
            } else {
                "not_attempted"
            }
        );
        if explicit {
            assert_eq!(error["error"]["operations"][0]["actor"], "bob");
        } else {
            assert_eq!(error["error"]["code"], "auth_error");
            assert!(stderr(&output).contains("GB_WEB_USER"));
        }
        assert!(!stderr(&output).contains("Notice:"));
        server.join().unwrap();
    }
}

#[test]
fn state_form_http_200_or_failed_readback_never_reports_success() {
    for prefix in ["", "/bucket"] {
        for status in ["200 OK", "500 Internal Server Error"] {
            let env = GbTestEnv::new();
            let mut script = state_preflight(prefix);
            script.extend([
                token_identity(prefix),
                signin(prefix),
                token_identity(prefix),
                ScriptedResponse::html(
                    &format!("POST {prefix}/alice/project/issue_comments/state HTTP/1.1"),
                    "200 OK",
                    "<form>validation error</form>",
                ),
                api(prefix, "GET", ISSUE, status, issue("open")),
            ]);
            let (port, server) = spawn_scripted_server(script);
            let output = env
                .repo_api_command(format!("127.0.0.1:{port}{prefix}"), "alice/project")
                .env("GB_USER", "alice")
                .env("GB_PASSWORD", "dummy")
                .args(["--json-errors", "issue", "close", "7"])
                .output()
                .unwrap();
            assert_failure(&output);
            assert!(output.stdout.is_empty());
            let error: Value = serde_json::from_slice(&output.stderr).unwrap();
            assert_eq!(error["error"]["operations"][0]["status"], "unverified");
            assert_eq!(error["error"]["outcome"], "unverified");
            if status.starts_with('5') {
                assert_eq!(error["error"]["code"], "api_error");
                assert_eq!(error["error"]["status"], 500);
            }
            server.join().unwrap();
        }
    }
}

#[test]
fn second_web_edit_failure_records_confirmed_and_unattempted_fields() {
    let env = GbTestEnv::new();
    let mut script = state_preflight("/bucket");
    let mut titled = issue("open");
    titled["title"] = json!("New");
    script.insert(
        1,
        api(
            "/bucket",
            "GET",
            "repos/alice/project/labels/urgent",
            "200 OK",
            json!({"name":"urgent"}),
        ),
    );
    script.extend([
        token_identity("/bucket"),
        signin("/bucket"),
        token_identity("/bucket"),
        ScriptedResponse::html(
            "POST /bucket/alice/project/issues/edit_title/7 HTTP/1.1",
            "200 OK",
            "updated",
        ),
        api("/bucket", "GET", ISSUE, "200 OK", titled),
        ScriptedResponse::html(
            "POST /bucket/alice/project/issues/edit/7 HTTP/1.1",
            "403 Forbidden",
            "denied",
        ),
    ]);
    let (port, server) = spawn_scripted_server(script);
    let output = env
        .repo_api_command(format!("127.0.0.1:{port}/bucket"), "alice/project")
        .env("GB_USER", "alice")
        .env("GB_PASSWORD", "dummy")
        .args([
            "--json-errors",
            "issue",
            "edit",
            "7",
            "--title",
            "New",
            "--body",
            "New body",
            "--state",
            "closed",
            "--add-label",
            "urgent",
        ])
        .output()
        .unwrap();
    assert_failure(&output);
    assert!(output.stdout.is_empty());
    let error: Value = serde_json::from_slice(&output.stderr).unwrap();
    let ops = error["error"]["operations"].as_array().unwrap();
    let state = |name| {
        ops.iter().find(|op| op["operation"] == name).unwrap()["status"]
            .as_str()
            .unwrap()
    };
    assert_eq!(state("issue.title"), "succeeded");
    assert_eq!(state("issue.body"), "failed");
    assert_eq!(state("issue.state"), "not_attempted");
    assert_eq!(state("issue.labels"), "not_attempted");
    assert_eq!(error["error"]["code"], "auth_error");
    server.join().unwrap();
}

#[test]
fn lossy_later_field_and_missing_target_fail_before_any_web_change() {
    let env = GbTestEnv::new();
    let (port, server) = spawn_scripted_server(state_preflight("/bucket"));
    let output = env
        .repo_api_command(format!("127.0.0.1:{port}/bucket"), "alice/project")
        .args(["issue", "edit", "7", "--title", "New", "--body", " padded "])
        .output()
        .unwrap();
    assert_failure(&output);
    assert!(stderr(&output).contains("No Web changes"));
    assert_eq!(server.join().unwrap().len(), 2);
    let (port, server) =
        spawn_scripted_server(vec![api("", "GET", ISSUE, "404 Not Found", json!({}))]);
    let output = env
        .repo_api_command(format!("127.0.0.1:{port}"), "alice/project")
        .args(["issue", "close", "7"])
        .output()
        .unwrap();
    assert_failure(&output);
    assert_eq!(server.join().unwrap().len(), 1);
}

#[test]
fn already_satisfied_state_is_confirmed_without_mutation_or_web_credentials() {
    let env = GbTestEnv::new();
    let (port, server) =
        spawn_scripted_server(vec![api("", "GET", ISSUE, "200 OK", issue("closed"))]);
    let output = env
        .repo_api_command(format!("127.0.0.1:{port}"), "alice/project")
        .env_remove("GB_PASSWORD")
        .args(["issue", "close", "7"])
        .output()
        .unwrap();
    assert_success(&output);
    assert_eq!(server.join().unwrap().len(), 1);
}

#[test]
fn redirect_failure_after_submission_is_unverified_even_for_login_or_404() {
    for signin_redirect in [false, true] {
        let env = GbTestEnv::new();
        let mut script = state_preflight("/bucket");
        let destination = if signin_redirect {
            "/bucket/signin"
        } else {
            "/bucket/missing"
        };
        script.extend([
            token_identity("/bucket"),
            signin("/bucket"),
            token_identity("/bucket"),
            ScriptedResponse::html(
                "POST /bucket/alice/project/issue_comments/state HTTP/1.1",
                "302 Found",
                "",
            )
            .with_header("location", destination),
            ScriptedResponse::html(
                &format!("GET {destination} HTTP/1.1"),
                if signin_redirect {
                    "200 OK"
                } else {
                    "404 Not Found"
                },
                "page",
            ),
        ]);
        let (port, server) = spawn_scripted_server(script);
        let output = env
            .repo_api_command(format!("127.0.0.1:{port}/bucket"), "alice/project")
            .env("GB_USER", "alice")
            .env("GB_PASSWORD", "dummy")
            .args(["--json-errors", "issue", "close", "7"])
            .output()
            .unwrap();
        assert_failure(&output);
        let error: Value = serde_json::from_slice(&output.stderr).unwrap();
        assert_eq!(error["error"]["operations"][0]["status"], "unverified");
        assert_eq!(
            error["error"]["code"],
            if signin_redirect {
                "auth_error"
            } else {
                "api_error"
            }
        );
        server.join().unwrap();
    }
}

#[test]
fn mutation_status_uses_redirect_history_instead_of_url_text() {
    let mut statuses = Vec::new();
    for (repo, redirect, fragment) in [
        ("project", false, false),
        ("案件", false, false),
        ("project", true, false),
        ("project", true, true),
    ] {
        let env = GbTestEnv::new();
        let encoded =
            percent_encoding::utf8_percent_encode(repo, percent_encoding::NON_ALPHANUMERIC)
                .to_string();
        let issue_path = format!("repos/alice/{encoded}/issues/7");
        let mutation_path = format!("/bucket/alice/{encoded}/issue_comments/state");
        let mut script = vec![
            api("/bucket", "GET", &issue_path, "200 OK", issue("open")),
            api("/bucket", "PATCH", &issue_path, "404 Not Found", json!({})),
            token_identity("/bucket"),
            signin("/bucket"),
            token_identity("/bucket"),
        ];
        let rejected = ScriptedResponse::html(
            &format!("POST {mutation_path} HTTP/1.1"),
            if redirect {
                "302 Found"
            } else {
                "403 Forbidden"
            },
            "",
        );
        script.push(if redirect {
            let location = if fragment {
                format!("{mutation_path}#result")
            } else {
                mutation_path.clone()
            };
            rejected.with_header("location", &location)
        } else {
            rejected
        });
        if redirect {
            script.push(ScriptedResponse::html(
                &format!("GET {mutation_path} HTTP/1.1"),
                "403 Forbidden",
                "denied after redirect",
            ));
        }
        let (port, server) = spawn_scripted_server(script);
        let output = env
            .repo_api_command(format!("127.0.0.1:{port}/bucket"), format!("alice/{repo}"))
            .env("GB_USER", "alice")
            .env("GB_PASSWORD", "dummy")
            .args(["--json-errors", "issue", "close", "7"])
            .output()
            .unwrap();
        assert_failure(&output);
        assert!(output.stdout.is_empty());
        let error: Value = serde_json::from_slice(&output.stderr).unwrap();
        assert_eq!(error["error"]["code"], "auth_error");
        statuses.push(
            error["error"]["operations"][0]["status"]
                .as_str()
                .unwrap()
                .to_owned(),
        );
        server.join().unwrap();
    }
    assert_eq!(statuses, ["failed", "failed", "unverified", "unverified"]);
}

#[test]
fn signin_rejects_encoded_redirects_that_escape_the_deployment() {
    for destination in ["/bucket/..%2Foutside", "/bucket/%252e%252e%252foutside"] {
        let env = GbTestEnv::new();
        let mut script = state_preflight("/bucket");
        script.extend([
            token_identity("/bucket"),
            ScriptedResponse::html("POST /bucket/signin HTTP/1.1", "307 Temporary Redirect", "")
                .with_header("location", destination),
        ]);
        let (port, server) = spawn_scripted_server(script);
        let output = env
            .repo_api_command(format!("127.0.0.1:{port}/bucket"), "alice/project")
            .env("GB_USER", "alice")
            .env("GB_PASSWORD", "must-not-be-forwarded")
            .args(["--json-errors", "issue", "close", "7"])
            .output()
            .unwrap();
        assert_failure(&output);
        assert!(output.stdout.is_empty());
        let error: Value = serde_json::from_slice(&output.stderr).unwrap();
        assert_eq!(error["error"]["code"], "auth_error", "{}", stderr(&output));
        assert_eq!(error["error"]["operations"][0]["status"], "not_attempted");
        assert!(!stderr(&output).contains("must-not-be-forwarded"));
        server.join().unwrap();
    }
}
