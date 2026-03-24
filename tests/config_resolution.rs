use std::fs;
use std::io::{self, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::thread;
use std::time::{Duration, Instant};

use tempfile::tempdir;

const SERVER_TIMEOUT: Duration = Duration::from_secs(5);

fn write_config(dir: &std::path::Path, content: &str) {
    fs::create_dir_all(dir).unwrap();
    fs::write(dir.join("config.toml"), content).unwrap();
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

fn serve_json_once(
    expected_request_line: &str,
    expected_auth: &str,
    body: &str,
) -> (u16, thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let expected_request_line = expected_request_line.to_string();
    let expected_auth = expected_auth.to_ascii_lowercase();
    let body = body.to_string();

    let server = thread::spawn(move || {
        let mut stream = accept_with_timeout(listener);
        let mut request = Vec::new();
        let mut buf = [0_u8; 1024];
        loop {
            let read = match stream.read(&mut buf) {
                Ok(read) => read,
                Err(err)
                    if err.kind() == io::ErrorKind::TimedOut
                        || err.kind() == io::ErrorKind::WouldBlock =>
                {
                    panic!("timed out while reading request from CLI");
                }
                Err(err) => panic!("failed to read request from CLI: {err}"),
            };
            if read == 0 {
                break;
            }
            request.extend_from_slice(&buf[..read]);
            if request.windows(4).any(|w| w == b"\r\n\r\n") {
                break;
            }
        }

        let request = String::from_utf8(request).unwrap();
        assert!(request.starts_with(&expected_request_line));
        assert!(request.to_ascii_lowercase().contains(&expected_auth));

        let response = format!(
            "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
            body.len(),
            body
        );
        stream.write_all(response.as_bytes()).unwrap();
    });

    (port, server)
}

#[test]
fn issue_list_rejects_invalid_state_before_api_call() {
    let temp = tempdir().unwrap();

    let output = gb_command()
        .current_dir(temp.path())
        .env("GB_CONFIG_DIR", temp.path())
        .env("GB_HOST", "gitbucket.example.com")
        .env("GB_REPO", "alice/my-repo")
        .env("GB_TOKEN", "env-token")
        .args(["issue", "list", "--state", "draft"])
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Invalid state 'draft'. Expected one of: open, closed, all"));
}

#[test]
fn pr_list_rejects_invalid_state_before_api_call() {
    let temp = tempdir().unwrap();

    let output = gb_command()
        .current_dir(temp.path())
        .env("GB_CONFIG_DIR", temp.path())
        .env("GB_HOST", "gitbucket.example.com")
        .env("GB_REPO", "alice/my-repo")
        .env("GB_TOKEN", "env-token")
        .args(["pr", "list", "--state", "draft"])
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Invalid state 'draft'. Expected one of: open, closed, all"));
}

#[test]
fn auth_token_uses_gb_host_over_default_host() {
    let temp = tempdir().unwrap();
    write_config(
        temp.path(),
        r#"
default_host = "default.example.com"

[hosts."default.example.com"]
token = "default-token"
user = "default-user"
protocol = "https"

[hosts."env.example.com"]
token = "env-host-token"
user = "env-user"
protocol = "https"
"#,
    );

    let output = gb_command()
        .current_dir(temp.path())
        .env("GB_CONFIG_DIR", temp.path())
        .env("GB_HOST", "env.example.com")
        .args(["auth", "token"])
        .output()
        .unwrap();

    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        "env-host-token"
    );
}

#[test]
fn issue_list_uses_environment_precedence_for_host_repo_token_and_protocol() {
    let temp = tempdir().unwrap();
    write_config(
        temp.path(),
        r#"
default_host = "bad host"

[hosts."bad host"]
token = "file-token"
user = "file-user"
protocol = "https"
"#,
    );

    let (port, server) = serve_json_once(
        "GET /api/v3/repos/env-owner/env-repo/issues?state=open HTTP/1.1",
        "authorization: token env-token",
        r#"[{"number":1,"title":"From env","state":"open","labels":[]}]"#,
    );

    let output = gb_command()
        .current_dir(temp.path())
        .env("GB_CONFIG_DIR", temp.path())
        .env("GB_HOST", format!("127.0.0.1:{port}"))
        .env("GB_REPO", "env-owner/env-repo")
        .env("GB_TOKEN", "env-token")
        .env("GB_PROTOCOL", "http")
        .args(["issue", "list", "--state", "open", "--json"])
        .output()
        .unwrap();

    server.join().unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("\"title\": \"From env\""));
    assert!(stdout.contains("\"number\": 1"));
}
