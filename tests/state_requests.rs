use std::io::{self, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::thread;
use std::time::{Duration, Instant};

use tempfile::tempdir;

const SERVER_TIMEOUT: Duration = Duration::from_secs(5);

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
fn issue_list_sends_closed_state_query_parameter() {
    let temp = tempdir().unwrap();
    let (port, server) = serve_json_once(
        "GET /api/v3/repos/alice/project/issues?state=closed HTTP/1.1",
        "authorization: token test-token",
        "[]",
    );

    let output = gb_command()
        .current_dir(temp.path())
        .env("GB_CONFIG_DIR", temp.path())
        .env("GB_HOST", format!("127.0.0.1:{port}"))
        .env("GB_REPO", "alice/project")
        .env("GB_TOKEN", "test-token")
        .env("GB_PROTOCOL", "http")
        .args(["issue", "list", "--state", "closed", "--json"])
        .output()
        .unwrap();

    server.join().unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "[]");
}

#[test]
fn pr_list_sends_all_state_query_parameter() {
    let temp = tempdir().unwrap();
    let (port, server) = serve_json_once(
        "GET /api/v3/repos/alice/project/pulls?state=all HTTP/1.1",
        "authorization: token test-token",
        "[]",
    );

    let output = gb_command()
        .current_dir(temp.path())
        .env("GB_CONFIG_DIR", temp.path())
        .env("GB_HOST", format!("127.0.0.1:{port}"))
        .env("GB_REPO", "alice/project")
        .env("GB_TOKEN", "test-token")
        .env("GB_PROTOCOL", "http")
        .args(["pr", "list", "--state", "all", "--json"])
        .output()
        .unwrap();

    server.join().unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "[]");
}
