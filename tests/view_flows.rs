mod support;

use tempfile::tempdir;

use support::gb_cmd::gb_command;
use support::mock_http::{spawn_scripted_server, ScriptedResponse};

#[test]
fn repo_view_renders_repository_details() {
    let temp = tempdir().unwrap();
    let (port, server) = spawn_scripted_server(vec![ScriptedResponse::json(
        "GET /api/v3/repos/alice/demo HTTP/1.1",
        "200 OK",
        r#"{"name":"demo","full_name":"alice/demo","description":"CLI repo","html_url":"https://gitbucket.example.com/alice/demo","clone_url":"https://gitbucket.example.com/alice/demo.git","private":false,"fork":false,"default_branch":"trunk","watchers_count":3,"forks_count":1,"open_issues_count":2}"#,
    )]);

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
    assert_eq!(
        requests[0].headers.get("authorization").map(String::as_str),
        Some("token test-token")
    );
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
fn repo_view_uses_global_repo_argument() {
    let temp = tempdir().unwrap();
    let (port, server) = spawn_scripted_server(vec![ScriptedResponse::json(
        "GET /api/v3/repos/alice/demo HTTP/1.1",
        "200 OK",
        r#"{"name":"demo","full_name":"alice/demo","private":false,"fork":false}"#,
    )]);

    let output = gb_command()
        .current_dir(temp.path())
        .env("GB_CONFIG_DIR", temp.path())
        .env("GB_HOST", format!("127.0.0.1:{port}"))
        .env("GB_TOKEN", "test-token")
        .env("GB_PROTOCOL", "http")
        .env("NO_COLOR", "1")
        .args(["-R", "alice/demo", "repo", "view"])
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
}

#[test]
fn issue_view_with_comments_renders_details_and_comments() {
    let temp = tempdir().unwrap();
    let (port, server) = spawn_scripted_server(vec![
        ScriptedResponse::json(
            "GET /api/v3/repos/alice/project/issues/7 HTTP/1.1",
            "200 OK",
            r#"{"number":7,"title":"Bug report","body":"Body text","state":"open","user":{"login":"alice"},"labels":[{"name":"bug"},{"name":"urgent"}],"assignees":[{"login":"bob"},{"login":"carol"}],"milestone":{"number":3,"title":"v1.0","state":"open"},"created_at":"2026-03-24T00:00:00Z"}"#,
        ),
        ScriptedResponse::json(
            "GET /api/v3/repos/alice/project/issues/7/comments HTTP/1.1",
            "200 OK",
            r#"[{"id":1,"body":"First comment","user":{"login":"bob"},"created_at":"2026-03-25T00:00:00Z"}]"#,
        ),
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
    for request in &requests {
        assert_eq!(
            request.headers.get("authorization").map(String::as_str),
            Some("token test-token")
        );
    }
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
    assert!(stdout.contains("Assignees: bob, carol"));
    assert!(stdout.contains("Milestone: v1.0 (#3)"));
    assert!(stdout.contains("Body text"));
    assert!(stdout.contains("--- Comments ---"));
    assert!(stdout.contains("bob (2026-03-25T00:00:00Z)"));
    assert!(stdout.contains("First comment"));
}

#[test]
fn pr_view_with_comments_renders_details_and_comments() {
    let temp = tempdir().unwrap();
    let (port, server) = spawn_scripted_server(vec![
        ScriptedResponse::json(
            "GET /api/v3/repos/alice/project/pulls/5 HTTP/1.1",
            "200 OK",
            r#"{"number":5,"title":"Add feature","body":"PR body","state":"closed","merged":true,"user":{"login":"alice"},"head":{"ref":"feature/demo"},"base":{"ref":"main"},"created_at":"2026-03-24T00:00:00Z"}"#,
        ),
        ScriptedResponse::json(
            "GET /api/v3/repos/alice/project/issues/5/comments HTTP/1.1",
            "200 OK",
            r#"[{"id":1,"body":"Please rebase","user":{"login":"carol"},"created_at":"2026-03-25T00:00:00Z"}]"#,
        ),
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
    for request in &requests {
        assert_eq!(
            request.headers.get("authorization").map(String::as_str),
            Some("token test-token")
        );
    }
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
fn pr_view_json_prints_pull_request_payload() {
    let temp = tempdir().unwrap();
    let (port, server) = spawn_scripted_server(vec![ScriptedResponse::json(
        "GET /api/v3/repos/alice/project/pulls/5 HTTP/1.1",
        "200 OK",
        r#"{"number":5,"title":"Add feature","body":"PR body","state":"open","head":{"ref":"feature/demo"},"base":{"ref":"main"}}"#,
    )]);

    let output = gb_command()
        .current_dir(temp.path())
        .env("GB_CONFIG_DIR", temp.path())
        .env("GB_HOST", format!("127.0.0.1:{port}"))
        .env("GB_REPO", "alice/project")
        .env("GB_TOKEN", "test-token")
        .env("GB_PROTOCOL", "http")
        .args(["pr", "view", "5", "--json"])
        .output()
        .unwrap();

    let requests = server.join().unwrap();

    assert_eq!(requests.len(), 1);
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(stdout["number"], 5);
    assert_eq!(stdout["head"]["ref"], "feature/demo");
    assert_eq!(stdout["base"]["ref"], "main");
}

#[test]
fn pr_view_json_with_comments_includes_comments_array() {
    let temp = tempdir().unwrap();
    let (port, server) = spawn_scripted_server(vec![
        ScriptedResponse::json(
            "GET /api/v3/repos/alice/project/pulls/5 HTTP/1.1",
            "200 OK",
            r#"{"number":5,"title":"Add feature","body":"PR body","state":"open","head":{"ref":"feature/demo"},"base":{"ref":"main"}}"#,
        ),
        ScriptedResponse::json(
            "GET /api/v3/repos/alice/project/issues/5/comments HTTP/1.1",
            "200 OK",
            r#"[{"id":11,"body":"Please rebase","user":{"login":"carol"},"created_at":"2026-03-25T00:00:00Z"}]"#,
        ),
    ]);

    let output = gb_command()
        .current_dir(temp.path())
        .env("GB_CONFIG_DIR", temp.path())
        .env("GB_HOST", format!("127.0.0.1:{port}"))
        .env("GB_REPO", "alice/project")
        .env("GB_TOKEN", "test-token")
        .env("GB_PROTOCOL", "http")
        .args(["pr", "view", "5", "--comments", "--json"])
        .output()
        .unwrap();

    let requests = server.join().unwrap();

    assert_eq!(requests.len(), 2);
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(stdout["number"], 5);
    assert_eq!(stdout["comments"][0]["id"], 11);
    assert_eq!(stdout["comments"][0]["body"], "Please rebase");
    assert_eq!(stdout["comments"][0]["user"]["login"], "carol");
}

#[test]
fn repo_view_surfaces_api_404() {
    let temp = tempdir().unwrap();
    let (port, server) = spawn_scripted_server(vec![ScriptedResponse::json(
        "GET /api/v3/repos/alice/missing HTTP/1.1",
        "404 Not Found",
        r#"{"message":"missing repo"}"#,
    )]);

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
    assert_eq!(
        requests[0].headers.get("authorization").map(String::as_str),
        Some("token test-token")
    );
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("API error (404)"));
    assert!(stderr.contains("missing repo"));
}

#[test]
fn issue_view_surfaces_api_404() {
    let temp = tempdir().unwrap();
    let (port, server) = spawn_scripted_server(vec![ScriptedResponse::json(
        "GET /api/v3/repos/alice/project/issues/7 HTTP/1.1",
        "404 Not Found",
        r#"{"message":"missing issue"}"#,
    )]);

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
    assert_eq!(
        requests[0].headers.get("authorization").map(String::as_str),
        Some("token test-token")
    );
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("API error (404)"));
    assert!(stderr.contains("missing issue"));
}

#[test]
fn pr_view_surfaces_api_404() {
    let temp = tempdir().unwrap();
    let (port, server) = spawn_scripted_server(vec![ScriptedResponse::json(
        "GET /api/v3/repos/alice/project/pulls/5 HTTP/1.1",
        "404 Not Found",
        r#"{"message":"missing pr"}"#,
    )]);

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
    assert_eq!(
        requests[0].headers.get("authorization").map(String::as_str),
        Some("token test-token")
    );
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("API error (404)"));
    assert!(stderr.contains("missing pr"));
}

#[test]
fn pr_view_falls_back_to_list_when_single_pr_response_is_empty() {
    let temp = tempdir().unwrap();
    let (port, server) = spawn_scripted_server(vec![
        ScriptedResponse::json(
            "GET /api/v3/repos/alice/project/pulls/5 HTTP/1.1",
            "200 OK",
            "",
        ),
        ScriptedResponse::json(
            "GET /api/v3/repos/alice/project/pulls?state=all HTTP/1.1",
            "200 OK",
            r#"[{"number":5,"title":"Fallback PR","body":"PR body","state":"open","merged":false,"user":{"login":"alice"},"head":{"ref":"feature/demo"},"base":{"ref":"main"},"created_at":"2026-03-24T00:00:00Z"}]"#,
        ),
    ]);

    let output = gb_command()
        .current_dir(temp.path())
        .env("GB_CONFIG_DIR", temp.path())
        .env("GB_HOST", format!("127.0.0.1:{port}"))
        .env("GB_REPO", "alice/project")
        .env("GB_TOKEN", "test-token")
        .env("GB_PROTOCOL", "http")
        .env("NO_COLOR", "1")
        .args(["pr", "view", "5"])
        .output()
        .unwrap();

    let requests = server.join().unwrap();

    assert_eq!(requests.len(), 2);
    for request in &requests {
        assert_eq!(
            request.headers.get("authorization").map(String::as_str),
            Some("token test-token")
        );
    }
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Fallback PR #5"));
    assert!(stdout.contains("OPEN"));
    assert!(stdout.contains("feature/demo"));
}

#[test]
fn issue_view_json_prints_issue_payload() {
    let temp = tempdir().unwrap();
    let (port, server) = spawn_scripted_server(vec![ScriptedResponse::json(
        "GET /api/v3/repos/alice/project/issues/7 HTTP/1.1",
        "200 OK",
        r#"{"number":7,"title":"Bug report","body":"Body text","state":"open","user":{"login":"alice"},"labels":[],"assignees":[],"created_at":"2026-03-24T00:00:00Z"}"#,
    )]);

    let output = gb_command()
        .current_dir(temp.path())
        .env("GB_CONFIG_DIR", temp.path())
        .env("GB_HOST", format!("127.0.0.1:{port}"))
        .env("GB_REPO", "alice/project")
        .env("GB_TOKEN", "test-token")
        .env("GB_PROTOCOL", "http")
        .args(["issue", "view", "7", "--json"])
        .output()
        .unwrap();

    let requests = server.join().unwrap();

    assert_eq!(requests.len(), 1);
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(stdout["number"], 7);
    assert_eq!(stdout["title"], "Bug report");
    assert_eq!(stdout["state"], "open");
}
