mod support;

use serde_json::Value;
use tempfile::tempdir;

use support::gb_cmd::gb_command;
use support::mock_http::{spawn_scripted_server, spawn_server, ScriptedResponse};

#[test]
fn pr_list_includes_open_prs_visible_through_repo_issues() {
    let temp = tempdir().unwrap();
    let (port, server) = spawn_scripted_server(vec![
        ScriptedResponse::json(
            "GET /api/v3/repos/alice/project/pulls?state=open HTTP/1.1",
            "200 OK",
            r#"[{"number":1,"title":"Listed PR","state":"open","head":{"ref":"list-head"}}]"#,
        ),
        ScriptedResponse::json(
            "GET /api/v3/repos/alice/project/pulls/1 HTTP/1.1",
            "200 OK",
            r#"{"number":1,"title":"Listed PR","state":"open","head":{"ref":"feature/list","repo":{"name":"fork","full_name":"bob/fork","private":false}},"base":{"ref":"main","repo":{"name":"project","full_name":"alice/project","private":false}}}"#,
        ),
        ScriptedResponse::json(
            "GET /api/v3/repos/alice/project/issues?state=open HTTP/1.1",
            "200 OK",
            r#"[
                {"number":1,"title":"Listed PR","state":"open","pull_request":{}},
                {"number":2,"title":"Visible PR","state":"open","pull_request":{}},
                {"number":3,"title":"Plain issue","state":"open"}
            ]"#,
        ),
        ScriptedResponse::json(
            "GET /api/v3/repos/alice/project/pulls/2 HTTP/1.1",
            "200 OK",
            r#"{"number":2,"title":"Visible PR","state":"open"}"#,
        ),
    ]);

    let output = gb_command()
        .current_dir(temp.path())
        .env("GB_CONFIG_DIR", temp.path())
        .env("GB_HOST", format!("127.0.0.1:{port}"))
        .env("GB_REPO", "alice/project")
        .env("GB_TOKEN", "test-token")
        .env("GB_PROTOCOL", "http")
        .args(["pr", "list", "--json"])
        .output()
        .unwrap();

    let requests = server.join().unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(stdout.as_array().unwrap().len(), 2);
    assert_eq!(stdout[0]["number"], 1);
    assert_eq!(stdout[0]["head"]["ref"], "feature/list");
    assert_eq!(stdout[0]["head"]["repo"]["full_name"], "bob/fork");
    assert_eq!(stdout[1]["number"], 2);
    assert_eq!(requests.len(), 4);
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
fn pr_comment_sends_expected_payload() {
    let temp = tempdir().unwrap();
    let (port, server) = spawn_server(
        "200 OK",
        r#"{"id":12,"body":"Please rebase","html_url":"http://127.0.0.1/alice/project/pull/5#comment-12"}"#,
    );

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
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Added comment 12 on PR #5"),
        "stdout: {stdout}"
    );
    assert!(
        stdout.contains("URL: http://127.0.0.1/alice/project/pull/5#comment-12"),
        "stdout: {stdout}"
    );
}

#[test]
fn pr_comment_json_prints_comment_object() {
    let temp = tempdir().unwrap();
    let (port, server) = spawn_server(
        "200 OK",
        r#"{"id":12,"body":"Please rebase","html_url":"http://127.0.0.1/alice/project/pull/5#comment-12"}"#,
    );

    let output = gb_command()
        .current_dir(temp.path())
        .env("GB_CONFIG_DIR", temp.path())
        .env("GB_HOST", format!("127.0.0.1:{port}"))
        .env("GB_REPO", "alice/project")
        .env("GB_TOKEN", "test-token")
        .env("GB_PROTOCOL", "http")
        .args(["pr", "comment", "5", "--body", "Please rebase", "--json"])
        .output()
        .unwrap();

    let _request = server.join().unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(stdout["id"], 12);
    assert_eq!(stdout["body"], "Please rebase");
    assert_eq!(
        stdout["html_url"],
        "http://127.0.0.1/alice/project/pull/5#comment-12"
    );
}

#[test]
fn pr_comment_list_json_prints_comments() {
    let temp = tempdir().unwrap();
    let (port, server) = spawn_scripted_server(vec![ScriptedResponse::json(
        "GET /api/v3/repos/alice/project/issues/5/comments?per_page=100 HTTP/1.1",
        "200 OK",
        r#"[
            {"id":10,"body":"First","user":{"login":"alice"}},
            {"id":12,"body":"Second","user":{"login":"bob"}}
        ]"#,
    )]);

    let output = gb_command()
        .current_dir(temp.path())
        .env("GB_CONFIG_DIR", temp.path())
        .env("GB_HOST", format!("127.0.0.1:{port}"))
        .env("GB_REPO", "alice/project")
        .env("GB_TOKEN", "test-token")
        .env("GB_PROTOCOL", "http")
        .args(["pr", "comment", "list", "5", "--json"])
        .output()
        .unwrap();

    let requests = server.join().unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(requests.len(), 1);
    let stdout: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(stdout[0]["id"], 10);
    assert_eq!(stdout[0]["body"], "First");
    assert_eq!(stdout[1]["user"]["login"], "bob");
}

#[test]
fn pr_comment_edit_last_updates_authenticated_users_latest_comment() {
    let temp = tempdir().unwrap();
    let (port, server) = spawn_scripted_server(vec![
        ScriptedResponse::json(
            "GET /api/v3/user HTTP/1.1",
            "200 OK",
            r#"{"login":"alice"}"#,
        ),
        ScriptedResponse::json(
            "GET /api/v3/repos/alice/project/issues/5/comments?per_page=100 HTTP/1.1",
            "200 OK",
            r#"[
                {"id":10,"body":"Older","user":{"login":"alice"}},
                {"id":12,"body":"Latest","user":{"login":"alice"}}
            ]"#,
        ),
        ScriptedResponse::json(
            "PATCH /api/v3/repos/alice/project/issues/comments/12 HTTP/1.1",
            "200 OK",
            r#"{"id":12,"body":"Edited"}"#,
        ),
    ]);

    let output = gb_command()
        .current_dir(temp.path())
        .env("GB_CONFIG_DIR", temp.path())
        .env("GB_HOST", format!("127.0.0.1:{port}"))
        .env("GB_REPO", "alice/project")
        .env("GB_TOKEN", "test-token")
        .env("GB_PROTOCOL", "http")
        .args(["pr", "comment", "5", "--edit-last", "--body", "Edited"])
        .output()
        .unwrap();

    let requests = server.join().unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(requests.len(), 3);
    assert_eq!(requests[2].method, "PATCH");
    assert_eq!(
        requests[2].target,
        "/api/v3/repos/alice/project/issues/comments/12"
    );
    let body: Value = serde_json::from_str(&requests[2].body).unwrap();
    assert_eq!(body["body"], "Edited");
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("Edited comment 12 on PR #5"),
        "stdout: {}",
        String::from_utf8_lossy(&output.stdout)
    );
}

#[test]
fn pr_merge_accepts_gitbucket_enveloped_response() {
    let temp = tempdir().unwrap();
    let (port, server) = spawn_server(
        "200 OK",
        r#"{"status":200,"body":"{\"sha\":\"abc123\",\"merged\":true,\"message\":\"Pull Request successfully merged\"}","headers":{}}"#,
    );

    let output = gb_command()
        .current_dir(temp.path())
        .env("GB_CONFIG_DIR", temp.path())
        .env("GB_HOST", format!("127.0.0.1:{port}"))
        .env("GB_REPO", "alice/project")
        .env("GB_TOKEN", "test-token")
        .env("GB_PROTOCOL", "http")
        .args(["pr", "merge", "9", "-m", "merge body"])
        .output()
        .unwrap();

    let request = server.join().unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(request.method, "PUT");
    assert_eq!(request.target, "/api/v3/repos/alice/project/pulls/9/merge");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Merged pull request #9"));
}
