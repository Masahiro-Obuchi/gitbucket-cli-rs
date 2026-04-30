mod support;

use serde_json::Value;
use support::gb_cmd::GbTestEnv;
use support::mock_http::{spawn_scripted_server, spawn_server, ScriptedResponse};

#[test]
fn pr_create_sends_expected_payload() {
    let env = GbTestEnv::new();
    let (port, server) = spawn_server(
        "200 OK",
        r#"{"number":5,"title":"Add feature","state":"open"}"#,
    );

    let output = env
        .repo_api_command(format!("127.0.0.1:{port}"), "alice/project")
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
fn pr_create_supports_head_owner_and_prints_resolved_refs() {
    let env = GbTestEnv::new();
    let (port, server) = spawn_server(
        "200 OK",
        r#"{
            "number":5,
            "title":"Add feature",
            "state":"open",
            "html_url":"http://127.0.0.1/pulls/5",
            "head":{"ref":"feature/branch","repo":{"name":"project","full_name":"bob/project"}},
            "base":{"ref":"main","repo":{"name":"project","full_name":"alice/project"}}
        }"#,
    );

    let output = env
        .repo_api_command(format!("127.0.0.1:{port}"), "alice/project")
        .args([
            "pr",
            "create",
            "-t",
            "Add feature",
            "-b",
            "PR body",
            "--head",
            "feature/branch",
            "--head-owner",
            "bob",
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
    let body: Value = serde_json::from_str(&request.body).unwrap();
    assert_eq!(body["head"], "bob:feature/branch");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Head: bob/project:feature/branch"));
    assert!(stdout.contains("Base: alice/project:main"));
    assert!(
        stdout.contains(&format!(
            "URL: http://127.0.0.1:{port}/alice/project/pull/5"
        )),
        "stdout: {stdout}"
    );
}

#[test]
fn pr_create_json_prints_created_pull_request() {
    let env = GbTestEnv::new();
    let (port, server) = spawn_server(
        "200 OK",
        r#"{"number":5,"title":"Add feature","state":"open","head":{"ref":"feature/branch"},"base":{"ref":"main"}}"#,
    );

    let output = env
        .repo_api_command(format!("127.0.0.1:{port}"), "alice/project")
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
            "--json",
        ])
        .output()
        .unwrap();

    let _request = server.join().unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(stdout["number"], 5);
    assert_eq!(stdout["head"]["ref"], "feature/branch");
    assert_eq!(stdout["base"]["ref"], "main");
}

#[test]
fn pr_create_detect_existing_returns_matching_open_pull_request() {
    let env = GbTestEnv::new();
    let (port, server) = spawn_scripted_server(vec![ScriptedResponse::json(
        "GET /api/v3/repos/alice/project/pulls?state=open HTTP/1.1",
        "200 OK",
        r#"[
            {"number":7,"title":"Existing PR","state":"open","head":{"ref":"feature/branch","label":"alice:feature/branch","repo":{"name":"project","full_name":"alice/project"}},"base":{"ref":"main","repo":{"name":"project","full_name":"alice/project"}}}
        ]"#,
    )]);

    let output = env
        .repo_api_command(format!("127.0.0.1:{port}"), "alice/project")
        .args([
            "pr",
            "create",
            "--head",
            "feature/branch",
            "--base",
            "main",
            "--detect-existing",
        ])
        .output()
        .unwrap();

    let requests = server.join().unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(requests.len(), 1);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Found existing pull request #7: Existing PR"),
        "stdout: {stdout}"
    );
}

#[test]
fn pr_create_detect_existing_ignores_qualified_head_from_different_repo() {
    let env = GbTestEnv::new();
    let (port, server) = spawn_scripted_server(vec![
        ScriptedResponse::json(
            "GET /api/v3/repos/alice/project/pulls?state=open HTTP/1.1",
            "200 OK",
            r#"[
                {"number":7,"title":"Different repo PR","state":"open","head":{"ref":"feature/branch","repo":{"name":"other-project","full_name":"bob/other-project"}},"base":{"ref":"main","repo":{"name":"project","full_name":"alice/project"}}}
            ]"#,
        ),
        ScriptedResponse::json(
            "GET /api/v3/repos/alice/project/issues?state=open HTTP/1.1",
            "200 OK",
            "[]",
        ),
        ScriptedResponse::json(
            "POST /api/v3/repos/alice/project/pulls HTTP/1.1",
            "200 OK",
            r#"{"number":8,"title":"Add feature","state":"open","head":{"ref":"feature/branch","repo":{"name":"project","full_name":"bob/project"}},"base":{"ref":"main","repo":{"name":"project","full_name":"alice/project"}}}"#,
        ),
    ]);

    let output = env
        .repo_api_command(format!("127.0.0.1:{port}"), "alice/project")
        .args([
            "pr",
            "create",
            "-t",
            "Add feature",
            "-b",
            "PR body",
            "--head",
            "feature/branch",
            "--head-owner",
            "bob",
            "--base",
            "main",
            "--detect-existing",
        ])
        .output()
        .unwrap();

    let requests = server.join().unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(requests.len(), 3);
    assert_eq!(requests[2].method, "POST");
    let body: Value = serde_json::from_str(&requests[2].body).unwrap();
    assert_eq!(body["head"], "bob:feature/branch");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Created pull request #8: Add feature"),
        "stdout: {stdout}"
    );
}

#[test]
fn pr_create_detect_existing_continues_when_issue_fallback_fails() {
    let env = GbTestEnv::new();
    let (port, server) = spawn_scripted_server(vec![
        ScriptedResponse::json(
            "GET /api/v3/repos/alice/project/pulls?state=open HTTP/1.1",
            "200 OK",
            "[]",
        ),
        ScriptedResponse::json(
            "GET /api/v3/repos/alice/project/issues?state=open HTTP/1.1",
            "500 Internal Server Error",
            r#"{"message":"temporary issue list failure"}"#,
        ),
        ScriptedResponse::json(
            "POST /api/v3/repos/alice/project/pulls HTTP/1.1",
            "200 OK",
            r#"{"number":8,"title":"Add feature","state":"open","head":{"ref":"feature/branch"},"base":{"ref":"main"}}"#,
        ),
    ]);

    let output = env
        .repo_api_command(format!("127.0.0.1:{port}"), "alice/project")
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
            "--detect-existing",
        ])
        .output()
        .unwrap();

    let requests = server.join().unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(requests.len(), 3);
    assert_eq!(requests[2].method, "POST");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("skipping issue-list fallback"),
        "stderr: {stderr}"
    );
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("Created pull request #8: Add feature"),
        "stdout: {}",
        String::from_utf8_lossy(&output.stdout)
    );
}

#[test]
fn pr_create_detect_existing_preserves_create_error_when_recheck_fails() {
    let env = GbTestEnv::new();
    let (port, server) = spawn_scripted_server(vec![
        ScriptedResponse::json(
            "GET /api/v3/repos/alice/project/pulls?state=open HTTP/1.1",
            "200 OK",
            "[]",
        ),
        ScriptedResponse::json(
            "GET /api/v3/repos/alice/project/issues?state=open HTTP/1.1",
            "200 OK",
            "[]",
        ),
        ScriptedResponse::json(
            "POST /api/v3/repos/alice/project/pulls HTTP/1.1",
            "422 Unprocessable Entity",
            r#"{"message":"Validation failed: pull request already exists"}"#,
        ),
        ScriptedResponse::json(
            "GET /api/v3/repos/alice/project/pulls?state=open HTTP/1.1",
            "500 Internal Server Error",
            r#"{"message":"temporary list failure"}"#,
        ),
    ]);

    let output = env
        .repo_api_command(format!("127.0.0.1:{port}"), "alice/project")
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
            "--detect-existing",
        ])
        .output()
        .unwrap();

    let requests = server.join().unwrap();

    assert!(!output.status.success());
    assert_eq!(requests.len(), 4);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Validation failed: pull request already exists"),
        "stderr: {stderr}"
    );
    assert!(
        !stderr.contains("temporary list failure"),
        "stderr: {stderr}"
    );
}

#[test]
fn pr_create_detect_existing_finds_pull_request_from_issue_listing_gap() {
    let env = GbTestEnv::new();
    let (port, server) = spawn_scripted_server(vec![
        ScriptedResponse::json(
            "GET /api/v3/repos/alice/project/pulls?state=open HTTP/1.1",
            "200 OK",
            "[]",
        ),
        ScriptedResponse::json(
            "GET /api/v3/repos/alice/project/issues?state=open HTTP/1.1",
            "200 OK",
            r#"[{"number":7,"title":"Existing PR","state":"open","pull_request":{}}]"#,
        ),
        ScriptedResponse::json(
            "GET /api/v3/repos/alice/project/pulls/7 HTTP/1.1",
            "200 OK",
            r#"{"number":7,"title":"Existing PR","state":"open","head":{"ref":"feature/branch","label":"alice:feature/branch","repo":{"name":"project","full_name":"alice/project"}},"base":{"ref":"main","repo":{"name":"project","full_name":"alice/project"}}}"#,
        ),
    ]);

    let output = env
        .repo_api_command(format!("127.0.0.1:{port}"), "alice/project")
        .args([
            "pr",
            "create",
            "--head",
            "feature/branch",
            "--base",
            "main",
            "--detect-existing",
        ])
        .output()
        .unwrap();

    let requests = server.join().unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(requests.len(), 3);
    assert!(
        String::from_utf8_lossy(&output.stdout)
            .contains("Found existing pull request #7: Existing PR"),
        "stdout: {}",
        String::from_utf8_lossy(&output.stdout)
    );
}

#[test]
fn pr_create_detect_existing_fetches_details_when_list_item_lacks_head_identity() {
    let env = GbTestEnv::new();
    let (port, server) = spawn_scripted_server(vec![
        ScriptedResponse::json(
            "GET /api/v3/repos/alice/project/pulls?state=open HTTP/1.1",
            "200 OK",
            r#"[
                {"number":7,"title":"Existing PR","state":"open","head":{"ref":"feature/branch"},"base":{"ref":"main"}}
            ]"#,
        ),
        ScriptedResponse::json(
            "GET /api/v3/repos/alice/project/pulls/7 HTTP/1.1",
            "200 OK",
            r#"{"number":7,"title":"Existing PR","state":"open","head":{"ref":"feature/branch","label":"alice:feature/branch","repo":{"name":"project","full_name":"alice/project"}},"base":{"ref":"main","repo":{"name":"project","full_name":"alice/project"}}}"#,
        ),
    ]);

    let output = env
        .repo_api_command(format!("127.0.0.1:{port}"), "alice/project")
        .args([
            "pr",
            "create",
            "--head",
            "feature/branch",
            "--base",
            "main",
            "--detect-existing",
        ])
        .output()
        .unwrap();

    let requests = server.join().unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(requests.len(), 2);
    assert_eq!(requests[1].target, "/api/v3/repos/alice/project/pulls/7");
    assert!(
        String::from_utf8_lossy(&output.stdout)
            .contains("Found existing pull request #7: Existing PR"),
        "stdout: {}",
        String::from_utf8_lossy(&output.stdout)
    );
}

#[test]
fn pr_create_detect_existing_continues_when_pr_detail_fetch_fails() {
    let env = GbTestEnv::new();
    let (port, server) = spawn_scripted_server(vec![
        ScriptedResponse::json(
            "GET /api/v3/repos/alice/project/pulls?state=open HTTP/1.1",
            "200 OK",
            r#"[
                {"number":7,"title":"Incomplete PR","state":"open","head":{"ref":"feature/branch"},"base":{"ref":"main"}}
            ]"#,
        ),
        ScriptedResponse::json(
            "GET /api/v3/repos/alice/project/pulls/7 HTTP/1.1",
            "500 Internal Server Error",
            r#"{"message":"temporary PR detail failure"}"#,
        ),
        ScriptedResponse::json(
            "GET /api/v3/repos/alice/project/issues?state=open HTTP/1.1",
            "200 OK",
            "[]",
        ),
        ScriptedResponse::json(
            "POST /api/v3/repos/alice/project/pulls HTTP/1.1",
            "200 OK",
            r#"{"number":8,"title":"Add feature","state":"open","head":{"ref":"feature/branch"},"base":{"ref":"main"}}"#,
        ),
    ]);

    let output = env
        .repo_api_command(format!("127.0.0.1:{port}"), "alice/project")
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
            "--detect-existing",
        ])
        .output()
        .unwrap();

    let requests = server.join().unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(requests.len(), 4);
    assert_eq!(requests[3].method, "POST");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("skipping pull request #7"),
        "stderr: {stderr}"
    );
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("Created pull request #8: Add feature"),
        "stdout: {}",
        String::from_utf8_lossy(&output.stdout)
    );
}

#[test]
fn pr_create_accepts_gitbucket_wrapped_json_response() {
    let env = GbTestEnv::new();
    let (port, server) = spawn_server(
        "200 OK",
        r#""{\"number\":9,\"title\":\"Wrapped PR\",\"state\":\"open\",\"merged\":false,\"head\":{\"ref\":\"feature/demo\"},\"base\":{\"ref\":\"main\"}}""#,
    );

    let output = env
        .repo_api_command(format!("127.0.0.1:{port}"), "alice/project")
        .args([
            "pr",
            "create",
            "-t",
            "Wrapped PR",
            "-b",
            "Body",
            "--head",
            "feature/demo",
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
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Created pull request #9"));
}
