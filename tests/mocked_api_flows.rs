use std::collections::HashMap;
use std::fs;
use std::io::{self, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::thread;
use std::time::{Duration, Instant};

use serde_json::Value;
use tempfile::tempdir;

const SERVER_TIMEOUT: Duration = Duration::from_secs(5);

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

fn accept_with_timeout(listener: TcpListener) -> TcpStream {
    listener.set_nonblocking(true).unwrap();
    let deadline = Instant::now() + SERVER_TIMEOUT;

    loop {
        match listener.accept() {
            Ok((stream, _)) => {
                stream.set_read_timeout(Some(SERVER_TIMEOUT)).unwrap();
                stream.set_write_timeout(Some(SERVER_TIMEOUT)).unwrap();
                return stream;
            }
            Err(err) if err.kind() == io::ErrorKind::WouldBlock => {
                if Instant::now() >= deadline {
                    panic!("timed out waiting for CLI to connect to mock server");
                }
                thread::sleep(Duration::from_millis(10));
            }
            Err(err) => panic!("failed to accept mock server connection: {err}"),
        }
    }
}

fn spawn_server(status_line: &str, body: &str) -> (u16, thread::JoinHandle<CapturedRequest>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let status_line = status_line.to_string();
    let body = body.to_string();

    let handle = thread::spawn(move || {
        let mut stream = accept_with_timeout(listener);
        let mut buffer = Vec::new();
        let mut chunk = [0_u8; 1024];
        let header_end;
        loop {
            let read = match stream.read(&mut chunk) {
                Ok(read) => read,
                Err(err)
                    if err.kind() == io::ErrorKind::TimedOut
                        || err.kind() == io::ErrorKind::WouldBlock =>
                {
                    panic!("timed out while reading request headers from CLI");
                }
                Err(err) => panic!("failed to read request headers from CLI: {err}"),
            };
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
            let read = match stream.read(&mut chunk) {
                Ok(read) => read,
                Err(err)
                    if err.kind() == io::ErrorKind::TimedOut
                        || err.kind() == io::ErrorKind::WouldBlock =>
                {
                    panic!("timed out while reading request body from CLI");
                }
                Err(err) => panic!("failed to read request body from CLI: {err}"),
            };
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

#[test]
fn repo_create_user_sends_expected_payload() {
    let temp = tempdir().unwrap();
    let (port, server) = spawn_server(
        "200 OK",
        r#"{"name":"demo","full_name":"alice/demo","private":true,"fork":false}"#,
    );

    let output = gb_command()
        .current_dir(temp.path())
        .env("GB_CONFIG_DIR", temp.path())
        .env("GB_HOST", format!("127.0.0.1:{port}"))
        .env("GB_TOKEN", "test-token")
        .env("GB_PROTOCOL", "http")
        .args([
            "repo",
            "create",
            "demo",
            "--description",
            "CLI repo",
            "--private",
            "--add-readme",
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
    assert_eq!(request.target, "/api/v3/user/repos");
    let body: Value = serde_json::from_str(&request.body).unwrap();
    assert_eq!(body["name"], "demo");
    assert_eq!(body["description"], "CLI repo");
    assert_eq!(body["private"], true);
    assert_eq!(body["auto_init"], true);
}

#[test]
fn repo_create_org_sends_expected_payload() {
    let temp = tempdir().unwrap();
    let (port, server) = spawn_server(
        "200 OK",
        r#"{"name":"demo","full_name":"my-org/demo","private":false,"fork":false}"#,
    );

    let output = gb_command()
        .current_dir(temp.path())
        .env("GB_CONFIG_DIR", temp.path())
        .env("GB_HOST", format!("127.0.0.1:{port}"))
        .env("GB_TOKEN", "test-token")
        .env("GB_PROTOCOL", "http")
        .args([
            "repo",
            "create",
            "demo",
            "--org",
            "my-org",
            "--description",
            "Org repo",
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
    assert_eq!(request.target, "/api/v3/orgs/my-org/repos");
    let body: Value = serde_json::from_str(&request.body).unwrap();
    assert_eq!(body["name"], "demo");
    assert_eq!(body["description"], "Org repo");
    assert_eq!(body["private"], false);
    assert_eq!(body["auto_init"], false);
}

#[test]
fn repo_fork_posts_empty_json_body() {
    let temp = tempdir().unwrap();
    let (port, server) = spawn_server(
        "200 OK",
        r#"{"name":"project","full_name":"bob/project","private":false,"fork":true}"#,
    );

    let output = gb_command()
        .current_dir(temp.path())
        .env("GB_CONFIG_DIR", temp.path())
        .env("GB_HOST", format!("127.0.0.1:{port}"))
        .env("GB_TOKEN", "test-token")
        .env("GB_PROTOCOL", "http")
        .args(["repo", "fork", "alice/project"])
        .output()
        .unwrap();

    let request = server.join().unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(request.method, "POST");
    assert_eq!(request.target, "/api/v3/repos/alice/project/forks");
    let body: Value = serde_json::from_str(&request.body).unwrap();
    assert_eq!(body, serde_json::json!({}));
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
