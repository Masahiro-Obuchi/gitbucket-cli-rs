mod support;

use std::io::{Read, Write};
use std::net::TcpListener;
use std::process::Output;
use std::thread;
use std::time::{Duration, Instant};

use serde_json::{json, Value};
use support::gb_cmd::{assert_failure, assert_success, GbTestEnv};
use support::mock_http::{spawn_scripted_server, spawn_server, ScriptedResponse};

fn error(output: &Output) -> Value {
    assert_failure(output);
    assert!(output.stdout.is_empty());
    serde_json::from_slice::<Value>(&output.stderr).unwrap()["error"].clone()
}

#[derive(Clone, Copy)]
enum Fault {
    Headers,
    Body,
    Disconnect,
    Trickle,
    Tls,
}

fn faulty_server(fault: Fault) -> (u16, thread::JoinHandle<usize>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let thread = thread::spawn(move || {
        listener.set_nonblocking(true).unwrap();
        let started = Instant::now();
        let mut stream = loop {
            match listener.accept() {
                Ok((stream, _)) => break stream,
                Err(_) if started.elapsed() < Duration::from_secs(10) => {
                    thread::sleep(Duration::from_millis(10))
                }
                Err(error) => panic!("client did not connect: {error}"),
            }
        };
        stream.set_nonblocking(false).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        if !matches!(fault, Fault::Tls) {
            let mut bytes = Vec::new();
            loop {
                let mut byte = [0];
                if stream.read(&mut byte).unwrap_or(0) == 0 {
                    break;
                }
                bytes.push(byte[0]);
                if bytes.ends_with(b"\r\n\r\n") {
                    break;
                }
            }
        }
        if matches!(fault, Fault::Body | Fault::Disconnect | Fault::Trickle) {
            stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 100\r\nConnection: close\r\n\r\n{").unwrap();
        }
        match fault {
            Fault::Disconnect => {}
            Fault::Trickle => {
                for _ in 0..8 {
                    thread::sleep(Duration::from_millis(60));
                    if stream.write_all(b" ").is_err() {
                        break;
                    }
                }
            }
            _ => thread::sleep(Duration::from_millis(600)),
        }
        drop(stream);
        // A retry can be queued even when no second request was serviced.
        let mut count = 1;
        while let Ok((_stream, _)) = listener.accept() {
            count += 1;
        }
        count
    });
    (port, thread)
}

#[test]
fn stalled_headers_and_body_keep_http_cause_status_and_write_uncertainty() {
    for fault in [
        Fault::Headers,
        Fault::Body,
        Fault::Disconnect,
        Fault::Trickle,
    ] {
        for write in [false, true] {
            let env = GbTestEnv::new();
            let (port, server) = faulty_server(fault);
            let started = Instant::now();
            let output = env
                .api_command(format!("127.0.0.1:{port}"))
                .env("GB_HTTP_TIMEOUT", "0.25")
                .env(
                    "GB_HTTP_READ_TIMEOUT",
                    if matches!(fault, Fault::Trickle) {
                        "2"
                    } else {
                        "0.12"
                    },
                )
                .args([
                    "--json-errors",
                    "api",
                    "user",
                    "-X",
                    if write { "POST" } else { "GET" },
                ])
                .output()
                .unwrap();
            assert!(started.elapsed() < Duration::from_secs(4));
            let e = error(&output);
            assert_eq!(e["code"], "http_error");
            assert_eq!(
                e["cause"],
                if matches!(fault, Fault::Disconnect) {
                    "body_read_failed"
                } else {
                    "timeout"
                }
            );
            if !matches!(fault, Fault::Headers) {
                assert_eq!(e["status"], 200);
            }
            if write {
                assert_eq!(e["outcome"], "unverified");
            } else {
                assert!(e.get("outcome").is_none());
            }
            assert!(e["message"].as_str().unwrap().contains("/api/v3/user"));
            assert_eq!(server.join().unwrap(), 1, "requests must not be retried");
        }
    }
}

#[test]
fn tls_handshake_stall_is_bounded_by_connect_timeout() {
    let env = GbTestEnv::new();
    let (port, server) = faulty_server(Fault::Tls);
    let output = env
        .api_command(format!("https://127.0.0.1:{port}"))
        .env("GB_HTTP_CONNECT_TIMEOUT", "0.12")
        .env("GB_HTTP_READ_TIMEOUT", "2")
        .env("GB_HTTP_TIMEOUT", "3")
        .args(["--json-errors", "api", "user"])
        .output()
        .unwrap();
    let e = error(&output);
    assert_eq!(e["code"], "http_error");
    assert_eq!(e["cause"], "timeout");
    assert!(e.get("status").is_none());
    assert_eq!(server.join().unwrap(), 1);
}

#[test]
fn typed_pr_body_disconnect_is_not_replaced_by_list_fallback() {
    let env = GbTestEnv::new();
    let (port, server) = faulty_server(Fault::Disconnect);
    let output = env
        .repo_api_command(format!("127.0.0.1:{port}"), "alice/project")
        .args(["--json-errors", "pr", "view", "7"])
        .output()
        .unwrap();
    let e = error(&output);
    assert_eq!(e["code"], "http_error");
    assert_eq!(e["cause"], "body_read_failed");
    assert_eq!(server.join().unwrap(), 1);
}

#[test]
fn typed_delete_reads_success_body_before_reporting_success() {
    let env = GbTestEnv::new();
    let (port, server) = faulty_server(Fault::Disconnect);
    let output = env
        .repo_api_command(format!("127.0.0.1:{port}"), "alice/project")
        .args(["--json-errors", "label", "delete", "bug", "--yes"])
        .output()
        .unwrap();
    let e = error(&output);
    assert_eq!(e["code"], "http_error");
    assert_eq!(e["cause"], "body_read_failed");
    assert_eq!(e["status"], 200);
    assert_eq!(e["outcome"], "unverified");
    assert_eq!(server.join().unwrap(), 1);
}

#[test]
fn error_details_are_bounded_and_do_not_echo_html_credentials_or_query_secrets() {
    for body in [
        "<html>password=unknown-private-value token=test-token</html>".into(),
        json!({"message":"echo test-token and configured-password"}).to_string(),
        "Authorization: Bearer unknown-private-value\nCookie: unknown-private-value".into(),
        format!("{}test-token", "日本語".repeat(6000)),
    ] {
        let env = GbTestEnv::new();
        let (port, server) = spawn_server("500 Internal Server Error", &body);
        let output = env
            .api_command(format!("127.0.0.1:{port}"))
            .env("GB_PASSWORD", "configured-password")
            .args(["--json-errors", "api", "user?access_token=query-secret"])
            .output()
            .unwrap();
        let e = error(&output);
        assert_eq!(e["code"], "api_error");
        assert_eq!(e["status"], 500);
        let text = String::from_utf8_lossy(&output.stderr);
        for secret in [
            "test-token",
            "configured-password",
            "unknown-private-value",
            "query-secret",
            "<html>",
        ] {
            assert!(!text.contains(secret), "{text}");
        }
        assert!(text.len() < 4000);
        server.join().unwrap();
    }
}

#[test]
fn raw_json_preserves_strings_and_status_body_objects() {
    for body in [
        json!("ordinary string"),
        json!("{\"number\":7}"),
        json!({"status":200,"body":"ordinary text"}),
        json!({"status":200,"body":"{\"merged\":true}","headers":{}}),
    ] {
        let env = GbTestEnv::new();
        let (port, server) = spawn_server("200 OK", &body.to_string());
        let output = env
            .api_command(format!("127.0.0.1:{port}"))
            .args(["api", "user"])
            .output()
            .unwrap();
        assert_success(&output);
        assert_eq!(
            serde_json::from_slice::<Value>(&output.stdout).unwrap(),
            body
        );
        server.join().unwrap();
    }
}

#[test]
fn typed_issue_preserves_ordinary_body_and_does_not_accept_pr_string_wrapper() {
    let issue =
        json!({"number":7,"title":"Issue","state":"open","status":200,"body":"ordinary body"});
    let env = GbTestEnv::new();
    let (port, server) = spawn_server("200 OK", &issue.to_string());
    let output = env
        .repo_api_command(format!("127.0.0.1:{port}"), "alice/project")
        .args(["issue", "view", "7", "--json"])
        .output()
        .unwrap();
    assert_success(&output);
    assert_eq!(
        serde_json::from_slice::<Value>(&output.stdout).unwrap()["body"],
        "ordinary body"
    );
    server.join().unwrap();
    let (port, server) = spawn_server("200 OK", &json!(issue.to_string()).to_string());
    let output = env
        .repo_api_command(format!("127.0.0.1:{port}"), "alice/project")
        .args([
            "--json-errors",
            "issue",
            "create",
            "--title",
            "Issue",
            "--body",
            "body",
        ])
        .output()
        .unwrap();
    let e = error(&output);
    assert_eq!(e["code"], "json_error");
    assert_eq!(e["outcome"], "unverified");
    server.join().unwrap();
}

#[test]
fn empty_raw_response_is_null_but_empty_typed_json_is_an_error() {
    for status in ["200 OK", "204 No Content"] {
        let env = GbTestEnv::new();
        let (port, server) = spawn_server(status, "");
        let output = env
            .api_command(format!("127.0.0.1:{port}"))
            .args(["api", "user"])
            .output()
            .unwrap();
        assert_success(&output);
        assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "null");
        server.join().unwrap();
        let (port, server) = spawn_server(status, "");
        let output = env
            .repo_api_command(format!("127.0.0.1:{port}"), "alice/project")
            .args(["--json-errors", "issue", "view", "7"])
            .output()
            .unwrap();
        assert_eq!(error(&output)["code"], "json_error");
        server.join().unwrap();
    }
}

#[test]
fn malformed_or_unrecognized_merge_envelopes_fail_without_retry() {
    for body in [
        json!({"status":201,"body":"{\"merged\":true}","headers":{}}),
        json!({"status":200,"body":"{\"merged\":true}"}),
        json!({"status":200,"body":"{\"merged\":true}","headers":{},"extra":true}),
        json!({"status":200,"body":"broken","headers":{}}),
    ] {
        let env = GbTestEnv::new();
        let (port, server) = spawn_scripted_server(vec![
            ScriptedResponse::json(
                "GET /api/v3/repos/alice/project/pulls/7 HTTP/1.1",
                "200 OK",
                r#"{"number":7,"title":"PR","state":"open"}"#,
            ),
            ScriptedResponse::json(
                "PUT /api/v3/repos/alice/project/pulls/7/merge HTTP/1.1",
                "200 OK",
                &body.to_string(),
            ),
        ]);
        let output = env
            .repo_api_command(format!("127.0.0.1:{port}"), "alice/project")
            .args(["--json-errors", "pr", "merge", "7"])
            .output()
            .unwrap();
        let e = error(&output);
        assert_eq!(e["code"], "json_error");
        assert_eq!(e["outcome"], "unverified");
        assert_eq!(server.join().unwrap().len(), 2);
    }
}

#[test]
fn invalid_deadlines_fail_before_network_or_git_mutations() {
    for name in [
        "GB_HTTP_CONNECT_TIMEOUT",
        "GB_HTTP_READ_TIMEOUT",
        "GB_HTTP_TIMEOUT",
        "GB_GIT_TIMEOUT",
    ] {
        for value in ["0", "-1", "NaN", "inf", "nonsense", "86401"] {
            let env = GbTestEnv::new();
            let mut cmd = env.api_command("127.0.0.1:9");
            cmd.env(name, value).arg("--json-errors");
            if name == "GB_GIT_TIMEOUT" {
                cmd.args(["repo", "clone", "file:///nonexistent", "must-not-exist"]);
            } else {
                cmd.args(["api", "user"]);
            }
            let e = error(&cmd.output().unwrap());
            assert_eq!(e["code"], "config_error");
            assert!(e["message"].as_str().unwrap().contains(name));
            assert!(!env.path().join("must-not-exist").exists());
        }
    }
}

#[test]
fn web_partial_response_keeps_transport_cause_and_distinguishes_login_from_write() {
    use support::mock_http::{accept_with_timeout, read_request, write_response};
    for identity in [false, true] {
        for status in ["200 OK", "403 Forbidden"] {
            let env = GbTestEnv::new();
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let port = listener.local_addr().unwrap().port();
            let server = thread::spawn(move || {
                let user = r#"{"login":"alice"}"#;
                let mut responses = vec![
                    ScriptedResponse::json(
                        "GET /bucket/api/v3/repos/alice/project/issues/7 HTTP/1.1",
                        "200 OK",
                        r#"{"number":7,"title":"Issue","state":"open"}"#,
                    ),
                    ScriptedResponse::json(
                        "PATCH /bucket/api/v3/repos/alice/project/issues/7 HTTP/1.1",
                        "404 Not Found",
                        "{}",
                    ),
                    ScriptedResponse::json("GET /bucket/api/v3/user HTTP/1.1", "200 OK", user),
                    ScriptedResponse::html("POST /bucket/signin HTTP/1.1", "200 OK", "signed in")
                        .with_header("set-cookie", "JSESSIONID=fault; Path=/"),
                ];
                if !identity {
                    responses.push(ScriptedResponse::json(
                        "GET /bucket/api/v3/user HTTP/1.1",
                        "200 OK",
                        user,
                    ));
                }
                for response in responses {
                    let mut stream = accept_with_timeout(&listener);
                    let request = read_request(&mut stream);
                    assert_eq!(
                        format!("{} {} HTTP/1.1", request.method, request.target),
                        response.expected_request_line
                    );
                    write_response(
                        &mut stream,
                        &response.status_line,
                        &response.headers,
                        &response.body,
                    );
                }
                let mut stream = accept_with_timeout(&listener);
                let request = read_request(&mut stream);
                assert_eq!(
                    request.target,
                    if identity {
                        "/bucket/api/v3/user"
                    } else {
                        "/bucket/alice/project/issue_comments/state"
                    }
                );
                stream.write_all(format!("HTTP/1.1 {status}\r\nContent-Length: 100\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n{{").as_bytes()).unwrap();
            });
            let output = env
                .repo_api_command(format!("127.0.0.1:{port}/bucket"), "alice/project")
                .env("GB_USER", "alice")
                .env("GB_PASSWORD", "dummy")
                .args(["--json-errors", "issue", "close", "7"])
                .output()
                .unwrap();
            let e = error(&output);
            assert_eq!(e["code"], "http_error");
            assert_eq!(e["cause"], "body_read_failed");
            assert_eq!(e["status"], if status.starts_with('2') { 200 } else { 403 });
            assert_eq!(
                e["operations"][0]["status"],
                if identity {
                    "not_attempted"
                } else {
                    "unverified"
                }
            );
            if !identity {
                assert_eq!(e["outcome"], "unverified");
            }
            server.join().unwrap();
        }
    }
}

#[test]
fn error_diagnostics_redact_credentials_supplied_in_json_request_body() {
    let env = GbTestEnv::new();
    let input = env.path().join("input.json");
    std::fs::write(&input, r#"{"password":"only-in-input-value"}"#).unwrap();
    let (port, server) = spawn_server(
        "422 Unprocessable Entity",
        r#"{"message":"rejected only-in-input-value"}"#,
    );
    let output = env
        .api_command(format!("127.0.0.1:{port}"))
        .args([
            "--json-errors",
            "api",
            "user",
            "--input",
            input.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    let e = error(&output);
    assert_eq!(e["code"], "api_error");
    assert!(!String::from_utf8_lossy(&output.stderr).contains("only-in-input-value"));
    server.join().unwrap();
}

#[test]
fn saved_diff_failures_do_not_reintroduce_query_secrets() {
    let env = GbTestEnv::new();
    let (port, server) = spawn_scripted_server(vec![
        ScriptedResponse::json(
            "GET /api/v3/repos/alice/project/pulls/7 HTTP/1.1",
            "200 OK",
            r#"{"number":7,"title":"PR","state":"closed","diff_url":"/alice/project/pull/7.diff?token=only-in-diff-url"}"#,
        ),
        ScriptedResponse::json(
            "GET /alice/project/pull/7.diff?token=only-in-diff-url HTTP/1.1",
            "404 Not Found",
            "{}",
        ),
        ScriptedResponse::json(
            "GET /alice/project/pull/7.diff HTTP/1.1",
            "404 Not Found",
            "{}",
        ),
    ]);
    let output = env
        .repo_api_command(format!("127.0.0.1:{port}"), "alice/project")
        .args(["--json-errors", "pr", "diff", "7", "--no-pager"])
        .output()
        .unwrap();
    let e = error(&output);
    assert_eq!(e["code"], "diff_unavailable");
    assert!(!String::from_utf8_lossy(&output.stderr).contains("only-in-diff-url"));
    assert_eq!(server.join().unwrap().len(), 3);
}
