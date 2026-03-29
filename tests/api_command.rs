mod support;

use std::fs;
use std::io::Write;
use std::process::Stdio;

use serde_json::Value;
use tempfile::tempdir;

use support::gb_cmd::gb_command;
use support::mock_http::spawn_server;

#[test]
fn api_get_normalizes_relative_endpoint() {
    let temp = tempdir().unwrap();
    let (port, server) = spawn_server("200 OK", r#"{"login":"alice"}"#);

    let output = gb_command()
        .current_dir(temp.path())
        .env("GB_CONFIG_DIR", temp.path())
        .env("GB_HOST", format!("127.0.0.1:{port}"))
        .env("GB_TOKEN", "test-token")
        .env("GB_PROTOCOL", "http")
        .args(["api", "user"])
        .output()
        .unwrap();

    let request = server.join().unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(request.method, "GET");
    assert_eq!(request.target, "/api/v3/user");
    assert_eq!(
        request.headers.get("authorization").map(String::as_str),
        Some("token test-token")
    );

    let payload: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(payload["login"], "alice");
}

#[test]
fn api_strips_api_prefix_from_endpoint() {
    let temp = tempdir().unwrap();
    let (port, server) = spawn_server("200 OK", r#"{"ok":true}"#);

    let output = gb_command()
        .current_dir(temp.path())
        .env("GB_CONFIG_DIR", temp.path())
        .env("GB_HOST", format!("127.0.0.1:{port}/gitbucket"))
        .env("GB_TOKEN", "test-token")
        .env("GB_PROTOCOL", "http")
        .args(["api", "/api/v3/user"])
        .output()
        .unwrap();

    let request = server.join().unwrap();

    assert!(output.status.success());
    assert_eq!(request.target, "/gitbucket/api/v3/user");
}

#[test]
fn api_preserves_similar_api_prefix_without_boundary() {
    let temp = tempdir().unwrap();
    let (port, server) = spawn_server("200 OK", r#"{"ok":true}"#);

    let output = gb_command()
        .current_dir(temp.path())
        .env("GB_CONFIG_DIR", temp.path())
        .env("GB_HOST", format!("127.0.0.1:{port}/gitbucket"))
        .env("GB_TOKEN", "test-token")
        .env("GB_PROTOCOL", "http")
        .args(["api", "/api/v30/user"])
        .output()
        .unwrap();

    let request = server.join().unwrap();

    assert!(output.status.success());
    assert_eq!(request.target, "/gitbucket/api/v3/api/v30/user");
}

#[test]
fn api_allows_absolute_url_within_same_api_base() {
    let temp = tempdir().unwrap();
    let (port, server) = spawn_server("200 OK", r#"{"login":"alice"}"#);
    let endpoint = format!("http://127.0.0.1:{port}/gitbucket/api/v3/user");

    let output = gb_command()
        .current_dir(temp.path())
        .env("GB_CONFIG_DIR", temp.path())
        .env("GB_HOST", format!("127.0.0.1:{port}/gitbucket"))
        .env("GB_TOKEN", "test-token")
        .env("GB_PROTOCOL", "http")
        .args(["api", &endpoint])
        .output()
        .unwrap();

    let request = server.join().unwrap();

    assert!(output.status.success());
    assert_eq!(request.method, "GET");
    assert_eq!(request.target, "/gitbucket/api/v3/user");
}

#[test]
fn api_rejects_cross_host_absolute_url_before_request() {
    let temp = tempdir().unwrap();

    let output = gb_command()
        .current_dir(temp.path())
        .env("GB_CONFIG_DIR", temp.path())
        .env("GB_HOST", "gitbucket.example.com/gitbucket")
        .env("GB_TOKEN", "test-token")
        .env("GB_PROTOCOL", "https")
        .args(["api", "https://evil.example.com/api/v3/user"])
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("configured GitBucket API base"),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn api_uses_post_by_default_when_input_is_present() {
    let temp = tempdir().unwrap();
    let body_path = temp.path().join("body.json");
    fs::write(&body_path, r#"{"title":"demo"}"#).unwrap();
    let (port, server) = spawn_server("200 OK", r#"{"number":1}"#);

    let output = gb_command()
        .current_dir(temp.path())
        .env("GB_CONFIG_DIR", temp.path())
        .env("GB_HOST", format!("127.0.0.1:{port}"))
        .env("GB_TOKEN", "test-token")
        .env("GB_PROTOCOL", "http")
        .args([
            "api",
            "repos/alice/demo/issues",
            "--input",
            body_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    let request = server.join().unwrap();

    assert!(output.status.success());
    assert_eq!(request.method, "POST");
    assert_eq!(request.target, "/api/v3/repos/alice/demo/issues");
    let body: Value = serde_json::from_str(&request.body).unwrap();
    assert_eq!(body["title"], "demo");
}

#[test]
fn api_reads_json_body_from_stdin() {
    let temp = tempdir().unwrap();
    let (port, server) = spawn_server("200 OK", r#"{"state":"closed"}"#);

    let mut command = gb_command();
    command
        .current_dir(temp.path())
        .env("GB_CONFIG_DIR", temp.path())
        .env("GB_HOST", format!("127.0.0.1:{port}"))
        .env("GB_TOKEN", "test-token")
        .env("GB_PROTOCOL", "http")
        .args([
            "api",
            "repos/alice/demo/issues/1",
            "-X",
            "PATCH",
            "--input",
            "-",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = command.spawn().unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(br#"{"state":"closed"}"#)
        .unwrap();
    let output = child.wait_with_output().unwrap();

    let request = server.join().unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(request.method, "PATCH");
    let body: Value = serde_json::from_str(&request.body).unwrap();
    assert_eq!(body["state"], "closed");
}

#[test]
fn api_delete_with_empty_success_body_prints_null() {
    let temp = tempdir().unwrap();
    let (port, server) = spawn_server("204 No Content", "");

    let output = gb_command()
        .current_dir(temp.path())
        .env("GB_CONFIG_DIR", temp.path())
        .env("GB_HOST", format!("127.0.0.1:{port}"))
        .env("GB_TOKEN", "test-token")
        .env("GB_PROTOCOL", "http")
        .args(["api", "repos/alice/demo", "-X", "DELETE"])
        .output()
        .unwrap();

    let request = server.join().unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(request.method, "DELETE");
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "null");
}

#[test]
fn api_rejects_invalid_json_input_before_request() {
    let temp = tempdir().unwrap();
    let body_path = temp.path().join("body.json");
    fs::write(&body_path, "not-json").unwrap();

    let output = gb_command()
        .current_dir(temp.path())
        .env("GB_CONFIG_DIR", temp.path())
        .env("GB_HOST", "127.0.0.1:9")
        .env("GB_TOKEN", "test-token")
        .env("GB_PROTOCOL", "http")
        .args(["api", "user", "--input", body_path.to_str().unwrap()])
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("JSON error:"),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}
