mod support;

use std::fs;

use serde_json::Value;
use tempfile::tempdir;

use support::gb_cmd::gb_command;
use support::mock_http::{spawn_scripted_server, spawn_server, ScriptedResponse};

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
fn auth_login_with_profile_saves_profile_scoped_host() {
    let temp = tempdir().unwrap();
    let (port, server) = spawn_server("200 OK", r#"{"login":"alice"}"#);

    let host = format!("127.0.0.1:{port}");
    let output = gb_command()
        .current_dir(temp.path())
        .env("GB_CONFIG_DIR", temp.path())
        .args([
            "--profile",
            "work",
            "auth",
            "login",
            "-H",
            &host,
            "-t",
            "profile-token",
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
    assert_eq!(request.target, "/api/v3/user");

    let config = fs::read_to_string(temp.path().join("config.toml")).unwrap();
    assert!(config.contains("[profiles.work]"));
    assert!(config.contains(&format!("default_host = \"{host}\"")));
    assert!(config.contains(&format!("[profiles.work.hosts.\"{host}\"]")));
    assert!(config.contains("token = \"profile-token\""));
    assert!(!config.contains("default_host = \"bad.example.com\""));
}

#[test]
fn auth_login_rejects_empty_profile_before_api_call() {
    let temp = tempdir().unwrap();

    let output = gb_command()
        .current_dir(temp.path())
        .env("GB_CONFIG_DIR", temp.path())
        .env("GB_PROFILE", "")
        .args([
            "auth",
            "login",
            "-H",
            "127.0.0.1:9",
            "-t",
            "secret-token",
            "--protocol",
            "http",
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("Profile name cannot be empty."),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        !temp.path().join("config.toml").exists(),
        "empty profile should not be written to config"
    );
}

#[test]
fn auth_status_with_profile_prints_only_that_profile_and_effective_actor() {
    let temp = tempdir().unwrap();
    fs::write(
        temp.path().join("config.toml"),
        r#"
[hosts."gitbucket.example.com"]
token = "global-token"
user = "global-user"
protocol = "https"

[profiles.work]
default_host = "gitbucket.example.com"
default_repo = "alice/project"

[profiles.work.hosts."gitbucket.example.com"]
token = "work-token"
user = "alice"
protocol = "https"

[profiles.sandbox]
default_host = "gitbucket.example.com"

[profiles.sandbox.hosts."gitbucket.example.com"]
token = "sandbox-token"
user = "bob"
protocol = "https"
"#,
    )
    .unwrap();

    let output = gb_command()
        .current_dir(temp.path())
        .env("GB_CONFIG_DIR", temp.path())
        .args(["auth", "status", "--profile", "work"])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Profile: work"));
    assert!(stdout.contains("Default repo: alice/project"));
    assert!(stdout.contains("Effective actor: alice @ gitbucket.example.com"));
    assert!(stdout.contains("profile credentials"));
    assert!(!stdout.contains("sandbox"));
    assert!(!stdout.contains("bob"));
    assert!(!stdout.contains("global-user"));
}

#[test]
fn auth_status_json_includes_structured_effective_actor() {
    let temp = tempdir().unwrap();
    fs::write(
        temp.path().join("config.toml"),
        r#"
default_profile = "work"

[hosts."gitbucket.example.com"]
token = "global-token"
user = "global-user"
protocol = "https"

[profiles.work]
default_host = "gitbucket.example.com"
default_repo = "alice/project"

[profiles.work.hosts."gitbucket.example.com"]
token = "work-token"
user = "alice"
protocol = "https"
"#,
    )
    .unwrap();

    let output = gb_command()
        .current_dir(temp.path())
        .env("GB_CONFIG_DIR", temp.path())
        .args(["auth", "status", "--json"])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let payload: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(payload["active_profile"], "work");
    assert_eq!(payload["effective_actor"]["host"], "gitbucket.example.com");
    assert_eq!(payload["effective_actor"]["user"], "alice");
    assert_eq!(payload["effective_actor"]["protocol"], "https");
    assert_eq!(payload["effective_actor"]["credential_source"], "profile");
    assert_eq!(
        payload["profiles"]["work"]["effective_actor"]["user"],
        "alice"
    );
}

#[test]
fn auth_status_json_global_credentials_includes_effective_actor() {
    let temp = tempdir().unwrap();
    fs::write(
        temp.path().join("config.toml"),
        r#"
[hosts."gitbucket.example.com"]
token = "global-token"
user = "global-user"
protocol = "https"
"#,
    )
    .unwrap();

    let output = gb_command()
        .current_dir(temp.path())
        .env("GB_CONFIG_DIR", temp.path())
        .args(["auth", "status", "--json", "-H", "gitbucket.example.com"])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let payload: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(payload["active_profile"].is_null());
    assert_eq!(payload["effective_actor"]["host"], "gitbucket.example.com");
    assert_eq!(payload["effective_actor"]["user"], "global-user");
    assert_eq!(payload["effective_actor"]["protocol"], "https");
    assert_eq!(payload["effective_actor"]["credential_source"], "global");
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
fn repo_fork_accepts_positional_repo_after_subcommand() {
    let temp = tempdir().unwrap();
    let (port, server) = spawn_server(
        "200 OK",
        r#"{"name":"project","full_name":"alice/project-fork","description":"","private":false,"fork":true}"#,
    );

    let output = gb_command()
        .current_dir(temp.path())
        .env("GB_CONFIG_DIR", temp.path())
        .env("GB_HOST", format!("127.0.0.1:{port}"))
        .env("GB_REPO", "ignored/from-env")
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
}

#[test]
fn repo_delete_yes_skips_confirmation_and_sends_delete_request() {
    let temp = tempdir().unwrap();
    let (port, server) = spawn_server("204 No Content", "");

    let output = gb_command()
        .current_dir(temp.path())
        .env("GB_CONFIG_DIR", temp.path())
        .env("GB_HOST", format!("127.0.0.1:{port}"))
        .env("GB_TOKEN", "test-token")
        .env("GB_PROTOCOL", "http")
        .args(["repo", "delete", "--yes", "alice/project"])
        .output()
        .unwrap();

    let request = server.join().unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(request.method, "DELETE");
    assert_eq!(request.target, "/api/v3/repos/alice/project");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Deleted repository alice/project"));
}

#[test]
fn repo_delete_falls_back_to_gitbucket_web_session() {
    let temp = tempdir().unwrap();
    let (port, server) = spawn_scripted_server(vec![
        ScriptedResponse::json(
            "DELETE /gitbucket/api/v3/repos/alice/project HTTP/1.1",
            "404 Not Found",
            r#"{"message":"Not Found"}"#,
        ),
        ScriptedResponse::html("POST /gitbucket/signin HTTP/1.1", "200 OK", "signed in")
            .with_header("set-cookie", "JSESSIONID=session456; Path=/; HttpOnly"),
        ScriptedResponse::html(
            "POST /gitbucket/alice/project/settings/delete HTTP/1.1",
            "200 OK",
            "deleted",
        ),
    ]);

    let output = gb_command()
        .current_dir(temp.path())
        .env("GB_CONFIG_DIR", temp.path())
        .env("GB_HOST", format!("127.0.0.1:{port}/gitbucket"))
        .env("GB_TOKEN", "test-token")
        .env("GB_PROTOCOL", "http")
        .env("GB_USER", "alice")
        .env("GB_PASSWORD", "secret-pass")
        .env("GB_REPO", "ignored/from-env")
        .args(["repo", "delete", "alice/project", "--yes"])
        .output()
        .unwrap();

    let requests = server.join().unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(requests.len(), 3);
    assert_eq!(
        requests[2].headers.get("cookie").map(String::as_str),
        Some("JSESSIONID=session456")
    );
    assert!(requests[2].body.is_empty());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Deleted repository alice/project"));
}

#[test]
fn repo_fork_falls_back_to_gitbucket_web_session() {
    let temp = tempdir().unwrap();
    let (port, server) = spawn_scripted_server(vec![
        ScriptedResponse::json(
            "POST /gitbucket/api/v3/repos/alice/project/forks HTTP/1.1",
            "404 Not Found",
            r#"{"message":"Not Found"}"#,
        ),
        ScriptedResponse::html("POST /gitbucket/signin HTTP/1.1", "200 OK", "signed in")
            .with_header("set-cookie", "JSESSIONID=session345; Path=/; HttpOnly"),
        ScriptedResponse::html(
            "POST /gitbucket/alice/project/fork HTTP/1.1",
            "200 OK",
            "forked",
        ),
    ]);

    let output = gb_command()
        .current_dir(temp.path())
        .env("GB_CONFIG_DIR", temp.path())
        .env("GB_HOST", format!("127.0.0.1:{port}/gitbucket"))
        .env("GB_TOKEN", "test-token")
        .env("GB_PROTOCOL", "http")
        .env("GB_USER", "alice")
        .env("GB_PASSWORD", "secret-pass")
        .env("GB_REPO", "ignored/from-env")
        .args(["repo", "fork", "alice/project", "--group", "my-group"])
        .output()
        .unwrap();

    let requests = server.join().unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(requests.len(), 3);
    assert_eq!(
        requests[2].headers.get("cookie").map(String::as_str),
        Some("JSESSIONID=session345")
    );
    assert!(requests[2].body.contains("account=my-group"));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Forked alice/project → my-group/project"));
}
