use crate::common::serve_json_once;
use crate::support::gb_cmd::GbTestEnv;
use crate::support::git_repo::init_bare_repo;

#[test]
fn repo_clone_full_url_does_not_require_gb_authentication() {
    let env = GbTestEnv::new();
    let remote = env.path().join("remote.git");
    init_bare_repo(&remote);

    let clone_url = format!("file://{}", remote.display());
    let output = env
        .command()
        .args(["repo", "clone", &clone_url, "cloned"])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(env.path().join("cloned").is_dir());
}

#[test]
fn repo_clone_full_url_rejects_missing_profile() {
    let env = GbTestEnv::new();
    let remote = env.path().join("remote.git");
    init_bare_repo(&remote);
    std::fs::write(env.path().join("config.toml"), "").unwrap();

    let clone_url = format!("file://{}", remote.display());
    let output = env
        .command()
        .args([
            "--profile",
            "missing",
            "repo",
            "clone",
            &clone_url,
            "cloned",
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("Profile 'missing' is not configured"),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!env.path().join("cloned").exists());
}

#[test]
fn json_errors_captures_git_clone_stderr() {
    let env = GbTestEnv::new();
    let missing_remote = env.path().join("missing.git");
    let clone_url = format!("file://{}", missing_remote.display());

    let output = env
        .command()
        .args(["--json-errors", "repo", "clone", &clone_url, "cloned"])
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(
        output.stdout.is_empty(),
        "stdout: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    let value: serde_json::Value = serde_json::from_str(stderr.trim()).unwrap();
    assert_eq!(value["error"]["code"], "error");
    assert!(
        value["error"]["message"]
            .as_str()
            .unwrap()
            .contains("git clone failed"),
        "stderr: {stderr}"
    );
    assert!(!env.path().join("cloned").exists());
}

#[test]
fn pr_merge_returns_non_zero_when_server_reports_not_merged() {
    let env = GbTestEnv::new();
    let (port, server) = serve_json_once(
        "PUT /api/v3/repos/alice/project/pulls/5/merge HTTP/1.1",
        "authorization: token test-token",
        r#"{"merged":false,"message":"merge conflict"}"#,
    );

    let output = env
        .repo_api_command(format!("127.0.0.1:{port}"), "alice/project")
        .args(["pr", "merge", "5"])
        .output()
        .unwrap();

    server.join().unwrap();

    assert!(
        !output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn repo_list_owner_uses_organization_endpoint_when_owner_is_org() {
    let env = GbTestEnv::new();
    let (port, server) = serve_json_once(
        "GET /api/v3/orgs/my-org/repos HTTP/1.1",
        "authorization: token test-token",
        "[]",
    );

    let output = env
        .api_command(format!("127.0.0.1:{port}"))
        .args(["repo", "list", "my-org", "--json"])
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
