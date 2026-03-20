use std::collections::HashMap;
use std::fs;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::thread;

use serde_json::Value;
use tempfile::tempdir;

#[derive(Debug)]
struct CapturedRequest {
    method: String,
    target: String,
    headers: HashMap<String, String>,
    body: String,
}

fn gb_command() -> std::process::Command {
    assert_cmd::cargo::CommandCargoExt::cargo_bin("gb").unwrap()
}

fn spawn_server(status_line: &str, body: &str) -> (u16, thread::JoinHandle<CapturedRequest>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let status_line = status_line.to_string();
    let body = body.to_string();

    let handle = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut buffer = Vec::new();
        let mut chunk = [0_u8; 1024];
        let header_end;
        loop {
            let read = stream.read(&mut chunk).unwrap();
            if read == 0 {
                panic!("connection closed before request headers were fully read");
            }
            buffer.extend_from_slice(&chunk[..read]);
            if let Some(pos) = buffer.windows(4).position(|w| w == b"\r\n\r\n") {
                header_end = pos + 4;
                break;
            }
        }

        let header_text = String::from_utf8(buffer[..header_end].to_vec()).unwrap();
        let mut lines = header_text.split("\r\n").filter(|line| !line.is_empty());
        let request_line = lines.next().unwrap();
        let mut request_parts = request_line.split_whitespace();
        let method = request_parts.next().unwrap().to_string();
        let target = request_parts.next().unwrap().to_string();

        let mut headers = HashMap::new();
        for line in lines {
            if let Some((name, value)) = line.split_once(':') {
                headers.insert(name.trim().to_ascii_lowercase(), value.trim().to_string());
            }
        }

        let content_length = headers
            .get("content-length")
            .and_then(|value| value.parse::<usize>().ok())
            .unwrap_or(0);
        while buffer.len() < header_end + content_length {
            let read = stream.read(&mut chunk).unwrap();
            if read == 0 {
                break;
            }
            buffer.extend_from_slice(&chunk[..read]);
        }
        let body_bytes =
            &buffer[header_end..header_end + content_length.min(buffer.len() - header_end)];
        let body_text = String::from_utf8(body_bytes.to_vec()).unwrap();

        let response = format!(
            "HTTP/1.1 {}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
            status_line,
            body.len(),
            body
        );
        stream.write_all(response.as_bytes()).unwrap();

        CapturedRequest {
            method,
            target,
            headers,
            body: body_text,
        }
    });

    (port, handle)
}

#[test]
fn auth_login_success_saves_default_host_and_user() {
    let temp = tempdir().unwrap();
    let (port, server) = spawn_server("200 OK", r#"{"login":"alice"}"#);

    let host = format!("127.0.0.1:{port}/gitbucket");
    let output = gb_command()
        .current_dir(temp.path())
        .env("GB_CONFIG_DIR", temp.path())
        .args([
            "auth",
            "login",
            "-H",
            &host,
            "-t",
            "secret-token",
            "--protocol",
            "http",
        ])
        .output()
        .unwrap();

    let request = server.join().unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(request.method, "GET");
    assert_eq!(request.target, "/gitbucket/api/v3/user");
    assert_eq!(
        request.headers.get("authorization").map(String::as_str),
        Some("token secret-token")
    );

    let config = fs::read_to_string(temp.path().join("config.toml")).unwrap();
    assert!(config.contains("default_host = \"127.0.0.1:"));
    assert!(config.contains("token = \"secret-token\""));
    assert!(config.contains("user = \"alice\""));
    assert!(config.contains("protocol = \"http\""));
}

#[test]
fn auth_login_maps_401_to_user_friendly_error() {
    let temp = tempdir().unwrap();
    let (port, server) = spawn_server("401 Unauthorized", r#"{"message":"bad credentials"}"#);

    let host = format!("127.0.0.1:{port}");
    let output = gb_command()
        .current_dir(temp.path())
        .env("GB_CONFIG_DIR", temp.path())
        .args([
            "auth",
            "login",
            "-H",
            &host,
            "-t",
            "secret-token",
            "--protocol",
            "http",
        ])
        .output()
        .unwrap();

    let request = server.join().unwrap();

    assert!(!output.status.success());
    assert_eq!(request.target, "/api/v3/user");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("HTTP 401"));
    assert!(stderr.contains("token was rejected"));
}

#[test]
fn auth_login_maps_404_to_base_path_hint() {
    let temp = tempdir().unwrap();
    let (port, server) = spawn_server("404 Not Found", r#"{"message":"not found"}"#);

    let host = format!("127.0.0.1:{port}");
    let output = gb_command()
        .current_dir(temp.path())
        .env("GB_CONFIG_DIR", temp.path())
        .args([
            "auth",
            "login",
            "-H",
            &host,
            "-t",
            "secret-token",
            "--protocol",
            "http",
        ])
        .output()
        .unwrap();

    let request = server.join().unwrap();

    assert!(!output.status.success());
    assert_eq!(request.target, "/api/v3/user");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("HTTP 404"));
    assert!(stderr.contains("/gitbucket"));
}

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
    assert_eq!(body["title"], "Bug report");
    assert_eq!(body["body"], "Body text");
    assert_eq!(body["labels"], serde_json::json!(["bug", "urgent"]));
    assert_eq!(body["assignees"], serde_json::json!(["alice", "bob"]));
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
