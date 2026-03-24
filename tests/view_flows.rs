use std::collections::HashMap;
use std::io::{self, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::thread;
use std::time::{Duration, Instant};

use tempfile::tempdir;

const SERVER_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Debug)]
struct CapturedRequest {
    method: String,
    target: String,
    headers: HashMap<String, String>,
}

#[derive(Debug)]
struct ExpectedResponse {
    request_line: String,
    auth_header: String,
    status_line: String,
    body: String,
}

fn gb_command() -> std::process::Command {
    assert_cmd::cargo::CommandCargoExt::cargo_bin("gb").unwrap()
}

fn accept_with_timeout(listener: &TcpListener) -> TcpStream {
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

fn read_request(stream: &mut TcpStream) -> CapturedRequest {
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

    CapturedRequest {
        method,
        target,
        headers,
    }
}

fn spawn_sequence_server(
    responses: Vec<ExpectedResponse>,
) -> (u16, thread::JoinHandle<Vec<CapturedRequest>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();

    let handle = thread::spawn(move || {
        let mut captured = Vec::new();

        for response in responses {
            let mut stream = accept_with_timeout(&listener);
            let request = read_request(&mut stream);
            let observed_request_line = format!("{} {} HTTP/1.1", request.method, request.target);
            assert_eq!(observed_request_line, response.request_line);
            assert_eq!(
                request.headers.get("authorization").map(String::as_str),
                Some(response.auth_header.as_str())
            );

            let raw_response = format!(
                "HTTP/1.1 {}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
                response.status_line,
                response.body.len(),
                response.body
            );
            stream.write_all(raw_response.as_bytes()).unwrap();
            captured.push(request);
        }

        captured
    });

    (port, handle)
}

#[test]
fn repo_view_renders_repository_details() {
    let temp = tempdir().unwrap();
    let (port, server) = spawn_sequence_server(vec![ExpectedResponse {
        request_line: "GET /api/v3/repos/alice/demo HTTP/1.1".into(),
        auth_header: "token test-token".into(),
        status_line: "200 OK".into(),
        body: r#"{"name":"demo","full_name":"alice/demo","description":"CLI repo","html_url":"https://gitbucket.example.com/alice/demo","clone_url":"https://gitbucket.example.com/alice/demo.git","private":false,"fork":false,"default_branch":"trunk","watchers_count":3,"forks_count":1,"open_issues_count":2}"#.into(),
    }]);

    let output = gb_command()
        .current_dir(temp.path())
        .env("GB_CONFIG_DIR", temp.path())
        .env("GB_HOST", format!("127.0.0.1:{port}"))
        .env("GB_TOKEN", "test-token")
        .env("GB_PROTOCOL", "http")
        .env("NO_COLOR", "1")
        .args(["repo", "view", "alice/demo"])
        .output()
        .unwrap();

    let requests = server.join().unwrap();

    assert_eq!(requests.len(), 1);
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("alice/demo"));
    assert!(stdout.contains("CLI repo"));
    assert!(stdout.contains("Visibility: Public"));
    assert!(stdout.contains("Default branch: trunk"));
    assert!(stdout.contains("URL: https://gitbucket.example.com/alice/demo"));
    assert!(stdout.contains("Clone: https://gitbucket.example.com/alice/demo.git"));
    assert!(stdout.contains("Stars: 3  Forks: 1  Issues: 2"));
}

#[test]
fn issue_view_with_comments_renders_details_and_comments() {
    let temp = tempdir().unwrap();
    let (port, server) = spawn_sequence_server(vec![
        ExpectedResponse {
            request_line: "GET /api/v3/repos/alice/project/issues/7 HTTP/1.1".into(),
            auth_header: "token test-token".into(),
            status_line: "200 OK".into(),
            body: r#"{"number":7,"title":"Bug report","body":"Body text","state":"open","user":{"login":"alice"},"labels":[{"name":"bug"},{"name":"urgent"}],"created_at":"2026-03-24T00:00:00Z"}"#.into(),
        },
        ExpectedResponse {
            request_line: "GET /api/v3/repos/alice/project/issues/7/comments HTTP/1.1".into(),
            auth_header: "token test-token".into(),
            status_line: "200 OK".into(),
            body: r#"[{"id":1,"body":"First comment","user":{"login":"bob"},"created_at":"2026-03-25T00:00:00Z"}]"#.into(),
        },
    ]);

    let output = gb_command()
        .current_dir(temp.path())
        .env("GB_CONFIG_DIR", temp.path())
        .env("GB_HOST", format!("127.0.0.1:{port}"))
        .env("GB_REPO", "alice/project")
        .env("GB_TOKEN", "test-token")
        .env("GB_PROTOCOL", "http")
        .env("NO_COLOR", "1")
        .args(["issue", "view", "7", "--comments"])
        .output()
        .unwrap();

    let requests = server.join().unwrap();

    assert_eq!(requests.len(), 2);
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Bug report #7"));
    assert!(stdout.contains("OPEN"));
    assert!(stdout.contains("Author: alice"));
    assert!(stdout.contains("Created: 2026-03-24T00:00:00Z"));
    assert!(stdout.contains("Labels: bug, urgent"));
    assert!(stdout.contains("Body text"));
    assert!(stdout.contains("--- Comments ---"));
    assert!(stdout.contains("bob (2026-03-25T00:00:00Z)"));
    assert!(stdout.contains("First comment"));
}

#[test]
fn pr_view_with_comments_renders_details_and_comments() {
    let temp = tempdir().unwrap();
    let (port, server) = spawn_sequence_server(vec![
        ExpectedResponse {
            request_line: "GET /api/v3/repos/alice/project/pulls/5 HTTP/1.1".into(),
            auth_header: "token test-token".into(),
            status_line: "200 OK".into(),
            body: r#"{"number":5,"title":"Add feature","body":"PR body","state":"closed","merged":true,"user":{"login":"alice"},"head":{"ref":"feature/demo"},"base":{"ref":"main"},"created_at":"2026-03-24T00:00:00Z"}"#.into(),
        },
        ExpectedResponse {
            request_line: "GET /api/v3/repos/alice/project/issues/5/comments HTTP/1.1".into(),
            auth_header: "token test-token".into(),
            status_line: "200 OK".into(),
            body: r#"[{"id":1,"body":"Please rebase","user":{"login":"carol"},"created_at":"2026-03-25T00:00:00Z"}]"#.into(),
        },
    ]);

    let output = gb_command()
        .current_dir(temp.path())
        .env("GB_CONFIG_DIR", temp.path())
        .env("GB_HOST", format!("127.0.0.1:{port}"))
        .env("GB_REPO", "alice/project")
        .env("GB_TOKEN", "test-token")
        .env("GB_PROTOCOL", "http")
        .env("NO_COLOR", "1")
        .args(["pr", "view", "5", "--comments"])
        .output()
        .unwrap();

    let requests = server.join().unwrap();

    assert_eq!(requests.len(), 2);
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Add feature #5"));
    assert!(stdout.contains("MERGED"));
    assert!(stdout.contains("main"));
    assert!(stdout.contains("feature/demo"));
    assert!(stdout.contains("Author: alice"));
    assert!(stdout.contains("Created: 2026-03-24T00:00:00Z"));
    assert!(stdout.contains("PR body"));
    assert!(stdout.contains("--- Comments ---"));
    assert!(stdout.contains("carol (2026-03-25T00:00:00Z)"));
    assert!(stdout.contains("Please rebase"));
}

#[test]
fn repo_view_surfaces_api_404() {
    let temp = tempdir().unwrap();
    let (port, server) = spawn_sequence_server(vec![ExpectedResponse {
        request_line: "GET /api/v3/repos/alice/missing HTTP/1.1".into(),
        auth_header: "token test-token".into(),
        status_line: "404 Not Found".into(),
        body: r#"{"message":"missing repo"}"#.into(),
    }]);

    let output = gb_command()
        .current_dir(temp.path())
        .env("GB_CONFIG_DIR", temp.path())
        .env("GB_HOST", format!("127.0.0.1:{port}"))
        .env("GB_TOKEN", "test-token")
        .env("GB_PROTOCOL", "http")
        .args(["repo", "view", "alice/missing"])
        .output()
        .unwrap();

    let requests = server.join().unwrap();

    assert_eq!(requests.len(), 1);
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("API error (404)"));
    assert!(stderr.contains("missing repo"));
}

#[test]
fn issue_view_surfaces_api_404() {
    let temp = tempdir().unwrap();
    let (port, server) = spawn_sequence_server(vec![ExpectedResponse {
        request_line: "GET /api/v3/repos/alice/project/issues/7 HTTP/1.1".into(),
        auth_header: "token test-token".into(),
        status_line: "404 Not Found".into(),
        body: r#"{"message":"missing issue"}"#.into(),
    }]);

    let output = gb_command()
        .current_dir(temp.path())
        .env("GB_CONFIG_DIR", temp.path())
        .env("GB_HOST", format!("127.0.0.1:{port}"))
        .env("GB_REPO", "alice/project")
        .env("GB_TOKEN", "test-token")
        .env("GB_PROTOCOL", "http")
        .args(["issue", "view", "7"])
        .output()
        .unwrap();

    let requests = server.join().unwrap();

    assert_eq!(requests.len(), 1);
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("API error (404)"));
    assert!(stderr.contains("missing issue"));
}

#[test]
fn pr_view_surfaces_api_404() {
    let temp = tempdir().unwrap();
    let (port, server) = spawn_sequence_server(vec![ExpectedResponse {
        request_line: "GET /api/v3/repos/alice/project/pulls/5 HTTP/1.1".into(),
        auth_header: "token test-token".into(),
        status_line: "404 Not Found".into(),
        body: r#"{"message":"missing pr"}"#.into(),
    }]);

    let output = gb_command()
        .current_dir(temp.path())
        .env("GB_CONFIG_DIR", temp.path())
        .env("GB_HOST", format!("127.0.0.1:{port}"))
        .env("GB_REPO", "alice/project")
        .env("GB_TOKEN", "test-token")
        .env("GB_PROTOCOL", "http")
        .args(["pr", "view", "5"])
        .output()
        .unwrap();

    let requests = server.join().unwrap();

    assert_eq!(requests.len(), 1);
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("API error (404)"));
    assert!(stderr.contains("missing pr"));
}
