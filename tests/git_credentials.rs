mod support;

use std::io::Write;
use std::process::Stdio;

use serde_json::json;
use support::gb_cmd::GbTestEnv;
use support::git_repo::{init_work_repo, run_git};
use support::mock_http::{spawn_scripted_server, ScriptedResponse};

fn pr_response(clone_url: &str) -> ScriptedResponse {
    ScriptedResponse::json(
        "GET /api/v3/repos/alice/demo/pulls/7 HTTP/1.1", "200 OK",
        &json!({"number":7,"title":"PR","state":"open","head":{"ref":"main","sha":"1".repeat(40),"repo":{"name":"demo","full_name":"alice/demo","clone_url":if clone_url.is_empty() { None } else { Some(clone_url) }}},"base":{"ref":"main","sha":"2".repeat(40)}}).to_string(),
    )
}

#[test]
fn private_fetch_uses_ephemeral_credentials_without_writing_token_to_repo() {
    // Use a URL relative to the selected API host, populated by a separate
    // server after the port is allocated. The existing scripted helper cannot
    // interpolate its own port, so use an origin remote instead of a clone URL.
    let (port, server) = spawn_scripted_server(vec![
        pr_response(""),
        ScriptedResponse::json(
            "GET /git/alice/demo.git/info/refs?service=git-upload-pack HTTP/1.1",
            "401 Unauthorized",
            "",
        )
        .with_header("www-authenticate", "Basic realm=GitBucket"),
        ScriptedResponse::json(
            "GET /git/alice/demo.git/info/refs?service=git-upload-pack HTTP/1.1",
            "404 Not Found",
            "",
        ),
    ]);
    let env = GbTestEnv::new();
    init_work_repo(env.path());
    run_git(
        env.path(),
        &[
            "remote",
            "add",
            "origin",
            &format!("http://127.0.0.1:{port}/git/alice/demo.git"),
        ],
    );
    let saved_credentials = env.path().join("saved-credentials");
    run_git(
        env.path(),
        &[
            "config",
            "credential.helper",
            &format!("store --file={}", saved_credentials.display()),
        ],
    );
    let config_before = std::fs::read(env.path().join(".git/config")).unwrap();
    // The helper command must also survive shell quoting in an installed path.
    let executable_dir = tempfile::tempdir().unwrap();
    let copied = executable_dir.path().join("gb helper's copy.exe");
    std::fs::copy(env.command().get_program(), &copied).unwrap();
    let output = std::process::Command::new(&copied)
        .current_dir(env.path())
        .env("GB_CONFIG_DIR", env.path())
        .env("GB_HOST", format!("127.0.0.1:{port}"))
        .env("GB_PROTOCOL", "http")
        .env("GB_TOKEN", "test-token")
        .env("GB_REPO", "alice/demo")
        .env("GB_USER", "alice")
        .args(["pr", "checkout", "7"])
        .output()
        .unwrap();
    let requests = server.join().unwrap();
    assert!(!output.status.success()); // Authentication succeeded; fixture has no Git repository.
    assert!(!requests[1].headers.contains_key("authorization"));
    assert_eq!(
        requests[2].headers.get("authorization").map(String::as_str),
        Some("Basic YWxpY2U6dGVzdC10b2tlbg==")
    );
    assert_eq!(
        std::fs::read(env.path().join(".git/config")).unwrap(),
        config_before
    );
    assert!(!String::from_utf8_lossy(&output.stderr).contains("test-token"));
    assert!(!saved_credentials.exists());
}

#[test]
fn url_rewrite_to_other_deployment_does_not_receive_token() {
    let (port, server) = spawn_scripted_server(vec![
        pr_response(""),
        ScriptedResponse::json(
            "GET /other/alice/demo.git/info/refs?service=git-upload-pack HTTP/1.1",
            "401 Unauthorized",
            "",
        )
        .with_header("www-authenticate", "Basic realm=GitBucket"),
    ]);
    let env = GbTestEnv::new();
    init_work_repo(env.path());
    run_git(
        env.path(),
        &[
            "remote",
            "add",
            "origin",
            &format!("http://127.0.0.1:{port}/git/alice/demo.git"),
        ],
    );
    run_git(
        env.path(),
        &[
            "config",
            &format!("url.http://127.0.0.1:{port}/other/.insteadOf"),
            &format!("http://127.0.0.1:{port}/git/"),
        ],
    );
    let output = env
        .repo_api_command(format!("127.0.0.1:{port}"), "alice/demo")
        .env("GB_USER", "alice")
        .args(["pr", "checkout", "7"])
        .output()
        .unwrap();
    let requests = server.join().unwrap();
    assert!(!output.status.success());
    assert!(!requests[1].headers.contains_key("authorization"));
    assert!(!String::from_utf8_lossy(&output.stderr).contains("test-token"));
}

#[test]
fn credential_helper_rejects_other_host_and_has_no_store_operation() {
    let env = GbTestEnv::new();
    for (operation, input, expected) in [
        (
            "get",
            "protocol=https\nhost=example.com\npath=git/a/r.git\n\n",
            "username=alice\npassword=dummy-secret\n\n",
        ),
        (
            "get",
            "protocol=https\nhost=other.example\npath=git/a/r.git\n\n",
            "quit=true\n\n",
        ),
        (
            "get",
            "protocol=https\nhost=example.com\npath=other/a/r.git\n\n",
            "quit=true\n\n",
        ),
        ("store", "password=do-not-store\n\n", ""),
        ("erase", "password=do-not-store\n\n", ""),
    ] {
        let mut child = env
            .command()
            .args(["--internal-git-credential", operation])
            .env("GB_INTERNAL_GIT_TARGET", "https://example.com/git/a/r.git")
            .env("GB_INTERNAL_GIT_USER", "alice")
            .env("GB_INTERNAL_GIT_TOKEN", "dummy-secret")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let input_result = child.stdin.take().unwrap().write_all(input.as_bytes());
        if let Err(error) = input_result {
            // store/erase may exit successfully without consuming stdin.
            // Still verify exit status, empty output, and non-persistence below.
            assert!(
                matches!(operation, "store" | "erase")
                    && error.kind() == std::io::ErrorKind::BrokenPipe,
                "{operation}: failed to send helper input: {error}"
            );
        }
        let output = child.wait_with_output().unwrap();
        assert!(
            output.status.success(),
            "operation {operation}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(String::from_utf8(output.stdout).unwrap(), expected);
        assert!(output.stderr.is_empty());
    }
    assert!(std::fs::read_dir(env.path()).unwrap().next().is_none());
}

#[test]
fn url_rewrite_to_other_origin_does_not_receive_token() {
    let (foreign_port, foreign) = spawn_scripted_server(vec![ScriptedResponse::json(
        "GET /git/alice/demo.git/info/refs?service=git-upload-pack HTTP/1.1",
        "401 Unauthorized",
        "",
    )
    .with_header("www-authenticate", "Basic realm=Foreign")]);
    let (port, api) = spawn_scripted_server(vec![pr_response("")]);
    let env = GbTestEnv::new();
    init_work_repo(env.path());
    run_git(
        env.path(),
        &[
            "remote",
            "add",
            "origin",
            &format!("http://127.0.0.1:{port}/git/alice/demo.git"),
        ],
    );
    run_git(
        env.path(),
        &[
            "config",
            &format!("url.http://127.0.0.1:{foreign_port}/.insteadOf"),
            &format!("http://127.0.0.1:{port}/"),
        ],
    );
    let output = env
        .repo_api_command(format!("127.0.0.1:{port}"), "alice/demo")
        .env("GB_USER", "alice")
        .args(["pr", "checkout", "7"])
        .output()
        .unwrap();
    api.join().unwrap();
    let requests = foreign.join().unwrap();
    assert!(!output.status.success());
    assert!(!requests[0].headers.contains_key("authorization"));
    assert!(!String::from_utf8_lossy(&output.stderr).contains("test-token"));
}

#[test]
fn authenticated_fetch_does_not_follow_redirects() {
    assert_fetch_does_not_follow_redirects(false, false);
}

#[test]
fn authenticated_fetch_does_not_follow_redirects_after_username_rewrite() {
    assert_fetch_does_not_follow_redirects(true, false);
}

#[test]
fn authenticated_fetch_overrides_redirect_setting_with_trailing_slash() {
    assert_fetch_does_not_follow_redirects(false, true);
}

fn assert_fetch_does_not_follow_redirects(rewrite_username: bool, trailing_slash: bool) {
    use std::net::TcpListener;
    use std::sync::mpsc;
    use std::time::Duration;
    let foreign = TcpListener::bind("127.0.0.1:0").unwrap();
    foreign.set_nonblocking(true).unwrap();
    let location = format!(
        "http://127.0.0.1:{}/stolen",
        foreign.local_addr().unwrap().port()
    );
    // Reply if a regression follows the redirect, so Git can exit and the
    // assertion fails instead of waiting forever on an unserved listener.
    let (stop, stopped) = mpsc::channel();
    let foreign = std::thread::spawn(move || {
        let mut connections = 0;
        loop {
            match foreign.accept() {
                Ok((mut stream, _)) => {
                    connections += 1;
                    stream.set_nonblocking(false).unwrap();
                    stream
                        .set_write_timeout(Some(Duration::from_secs(5)))
                        .unwrap();
                    let _ = stream.write_all(
                        b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                    );
                }
                Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                    match stopped.recv_timeout(Duration::from_millis(10)) {
                        Ok(()) | Err(mpsc::RecvTimeoutError::Disconnected) => return connections,
                        Err(mpsc::RecvTimeoutError::Timeout) => {}
                    }
                }
                Err(err) => panic!("redirect listener failed: {err}"),
            }
        }
    });
    let target = "GET /git/alice/demo.git/info/refs?service=git-upload-pack HTTP/1.1";
    let (port, server) = spawn_scripted_server(vec![
        pr_response(""),
        ScriptedResponse::json(target, "401 Unauthorized", "")
            .with_header("www-authenticate", "Basic realm=GitBucket"),
        ScriptedResponse::json(target, "302 Found", "").with_header("location", &location),
    ]);
    let env = GbTestEnv::new();
    init_work_repo(env.path());
    run_git(
        env.path(),
        &[
            "remote",
            "add",
            "origin",
            &format!("http://127.0.0.1:{port}/git/alice/demo.git"),
        ],
    );
    let configured_url = if rewrite_username {
        let rewritten = format!("http://alice@127.0.0.1:{port}/git/alice/demo.git");
        run_git(
            env.path(),
            &[
                "config",
                &format!("url.{rewritten}.insteadOf"),
                &format!("http://127.0.0.1:{port}/git/alice/demo.git"),
            ],
        );
        rewritten
    } else {
        format!("http://127.0.0.1:{port}/git/alice/demo.git")
    };
    let configured_url = if trailing_slash {
        format!("{configured_url}/")
    } else {
        configured_url
    };
    run_git(
        env.path(),
        &[
            "config",
            &format!("http.{configured_url}.followRedirects"),
            "true",
        ],
    );
    let output = env
        .repo_api_command(format!("127.0.0.1:{port}"), "alice/demo")
        .env("GB_USER", "alice")
        .args(["pr", "checkout", "7"])
        .output()
        .unwrap();
    stop.send(()).unwrap();
    let redirected = foreign.join().unwrap();
    let requests = server.join().unwrap();
    assert!(!output.status.success());
    assert!(requests[2].headers.contains_key("authorization"));
    assert_eq!(redirected, 0, "authenticated fetch followed a redirect");
}

#[test]
fn private_fetch_supports_percent_encoded_deployment_paths() {
    let mut pr = pr_response("");
    pr.expected_request_line = "GET /bucket%20site/api/v3/repos/alice/demo/pulls/7 HTTP/1.1".into();
    let target = "GET /bucket%20site/alice/demo.git/info/refs?service=git-upload-pack HTTP/1.1";
    let (port, server) = spawn_scripted_server(vec![
        pr,
        ScriptedResponse::json(target, "401 Unauthorized", "")
            .with_header("www-authenticate", "Basic realm=GitBucket"),
        ScriptedResponse::json(target, "404 Not Found", ""),
    ]);
    let env = GbTestEnv::new();
    init_work_repo(env.path());
    run_git(
        env.path(),
        &[
            "remote",
            "add",
            "origin",
            &format!("http://127.0.0.1:{port}/bucket%20site/alice/demo.git"),
        ],
    );
    let output = env
        .repo_api_command(format!("127.0.0.1:{port}/bucket%20site"), "alice/demo")
        .env("GB_USER", "alice")
        .args(["pr", "checkout", "7"])
        .output()
        .unwrap();
    let requests = server.join().unwrap();
    assert!(!output.status.success());
    assert_eq!(
        requests[2].headers.get("authorization").map(String::as_str),
        Some("Basic YWxpY2U6dGVzdC10b2tlbg==")
    );
}

#[test]
fn clone_uses_scoped_credentials_and_does_not_store_them() {
    let target = "GET /alice/demo.git/info/refs?service=git-upload-pack HTTP/1.1";
    let (port, server) = spawn_scripted_server(vec![
        ScriptedResponse::json(
            "GET /api/v3/repos/alice/demo HTTP/1.1",
            "200 OK",
            r#"{"name":"demo","full_name":"alice/demo","private":true}"#,
        ),
        ScriptedResponse::json(target, "401 Unauthorized", "")
            .with_header("www-authenticate", "Basic realm=GitBucket"),
        ScriptedResponse::json(target, "404 Not Found", ""),
    ]);
    let env = GbTestEnv::new();
    let output = env
        .api_command(format!("127.0.0.1:{port}"))
        .env("GB_USER", "alice")
        .args(["repo", "clone", "alice/demo"])
        .output()
        .unwrap();
    let requests = server.join().unwrap();
    assert!(!output.status.success());
    assert_eq!(
        requests[2].headers.get("authorization").map(String::as_str),
        Some("Basic YWxpY2U6dGVzdC10b2tlbg==")
    );
    assert!(!String::from_utf8_lossy(&output.stderr).contains("test-token"));
    assert!(!env.path().join("demo").exists());
}

#[test]
fn private_clone_supports_trailing_slash_in_api_url() {
    let target = "GET /git/alice/demo.git/info/refs?service=git-upload-pack HTTP/1.1";
    let (port, server) = spawn_scripted_server(vec![
        ScriptedResponse::json(
            "GET /api/v3/repos/alice/demo HTTP/1.1",
            "200 OK",
            r#"{"name":"demo","full_name":"alice/demo","private":true,"clone_url":"http://gitbucket:8080/git/alice/demo.git/"}"#,
        ),
        ScriptedResponse::json(target, "401 Unauthorized", "")
            .with_header("www-authenticate", "Basic realm=GitBucket"),
        ScriptedResponse::json(target, "404 Not Found", ""),
    ]);
    let env = GbTestEnv::new();
    let output = env
        .api_command(format!("127.0.0.1:{port}"))
        .env("GB_USER", "alice")
        .args(["repo", "clone", "alice/demo"])
        .output()
        .unwrap();
    let requests = server.join().unwrap();
    assert!(!output.status.success());
    assert_eq!(
        requests[2].headers.get("authorization").map(String::as_str),
        Some("Basic YWxpY2U6dGVzdC10b2tlbg==")
    );
}

#[test]
fn full_clone_url_password_is_rejected_without_disclosing_it() {
    let env = GbTestEnv::new();
    let output = env
        .command()
        .args([
            "repo",
            "clone",
            "https://alice:do-not-log-this@example.invalid/a/r.git",
        ])
        .output()
        .unwrap();
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Passwords in HTTP clone URLs are not supported"));
    assert!(!stderr.contains("do-not-log-this"));
}

#[test]
fn authenticated_url_cannot_be_rewritten_to_an_external_transport() {
    let env = GbTestEnv::new();
    init_work_repo(env.path());
    // The rewritten transport is usable: without the protocol restriction,
    // checkout would succeed. Do not use a nonexistent target as the control.
    let transport_dir = tempfile::tempdir().unwrap();
    let remote = transport_dir.path().join("local-transport");
    init_work_repo(&remote);
    support::git_repo::commit_readme(&remote, "local transport\n", "Seed transport");
    run_git(&remote, &["branch", "-M", "main"]);
    let sha = support::git_repo::git_output(&remote, &["rev-parse", "HEAD"]);
    let mut response = pr_response("");
    let mut payload: serde_json::Value = serde_json::from_str(&response.body).unwrap();
    payload["head"]["sha"] = json!(sha);
    payload["base"]["sha"] = json!(sha);
    response.body = payload.to_string();
    let (port, api) = spawn_scripted_server(vec![response]);
    let url = format!("http://127.0.0.1:{port}/git/alice/demo.git");
    run_git(env.path(), &["remote", "add", "origin", &url]);
    let file_url = url::Url::from_directory_path(&remote).unwrap();
    run_git(env.path(), &["config", "protocol.file.allow", "always"]);
    run_git(
        env.path(),
        &["config", &format!("url.{file_url}.insteadOf"), &url],
    );
    let output = env
        .repo_api_command(format!("127.0.0.1:{port}"), "alice/demo")
        .env("GB_USER", "alice")
        .args(["pr", "checkout", "7"])
        .output()
        .unwrap();
    api.join().unwrap();
    assert!(!output.status.success());
    assert!(!String::from_utf8_lossy(&output.stderr).contains("test-token"));
    assert!(!std::process::Command::new("git")
        .current_dir(env.path())
        .args(["cat-file", "-e", &format!("{sha}^{{commit}}")])
        .output()
        .unwrap()
        .status
        .success());
    // Positive control: the identical rewritten URL supplies the saved object
    // without gb's credential-bearing protocol restriction.
    run_git(env.path(), &["fetch", "--", &url, &sha]);
    assert_eq!(
        support::git_repo::git_output(env.path(), &["rev-parse", "FETCH_HEAD"]),
        sha
    );
}
