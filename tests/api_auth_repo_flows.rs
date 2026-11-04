mod support;

use std::fs;

use serde_json::Value;
use support::gb_cmd::GbTestEnv;
use support::mock_http::spawn_server;

#[test]
fn auth_login_success_saves_default_host_and_user() {
    let env = GbTestEnv::new();
    let (port, server) = spawn_server("200 OK", r#"{"login":"alice"}"#);

    let host = format!("127.0.0.1:{port}/gitbucket");
    let output = env
        .command()
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

    let config = fs::read_to_string(env.path().join("config.toml")).unwrap();
    assert!(config.contains("default_host = \"127.0.0.1:"));
    assert!(config.contains("token = \"secret-token\""));
    assert!(config.contains("user = \"alice\""));
    assert!(config.contains("protocol = \"http\""));
}

#[test]
fn auth_login_with_profile_saves_profile_scoped_host() {
    let env = GbTestEnv::new();
    let (port, server) = spawn_server("200 OK", r#"{"login":"alice"}"#);

    let host = format!("127.0.0.1:{port}");
    let output = env
        .command()
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

    let config = fs::read_to_string(env.path().join("config.toml")).unwrap();
    assert!(config.contains("[profiles.work]"));
    assert!(config.contains(&format!("default_host = \"{host}\"")));
    assert!(config.contains(&format!("[profiles.work.hosts.\"{host}\"]")));
    assert!(config.contains("token = \"profile-token\""));
    assert!(!config.contains("default_host = \"bad.example.com\""));
}

#[test]
fn auth_login_rejects_empty_profile_before_api_call() {
    let env = GbTestEnv::new();

    let output = env
        .command()
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
        !env.path().join("config.toml").exists(),
        "empty profile should not be written to config"
    );
}

#[test]
fn auth_status_with_profile_prints_only_that_profile_and_effective_actor() {
    let env = GbTestEnv::new();
    fs::write(
        env.path().join("config.toml"),
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

    let output = env
        .command()
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
    let env = GbTestEnv::new();
    fs::write(
        env.path().join("config.toml"),
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

    let output = env
        .command()
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
    let env = GbTestEnv::new();
    fs::write(
        env.path().join("config.toml"),
        r#"
[hosts."gitbucket.example.com"]
token = "global-token"
user = "global-user"
protocol = "https"
"#,
    )
    .unwrap();

    let output = env
        .command()
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
    let env = GbTestEnv::new();
    let (port, server) = spawn_server("401 Unauthorized", r#"{"message":"bad credentials"}"#);

    let host = format!("127.0.0.1:{port}");
    let output = env
        .command()
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
    let env = GbTestEnv::new();
    let (port, server) = spawn_server("404 Not Found", r#"{"message":"not found"}"#);

    let host = format!("127.0.0.1:{port}");
    let output = env
        .command()
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
    let env = GbTestEnv::new();
    let (port, server) = spawn_server(
        "200 OK",
        r#"{"name":"demo","full_name":"alice/demo","private":true,"fork":false}"#,
    );

    let output = env
        .api_command(format!("127.0.0.1:{port}"))
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
    let env = GbTestEnv::new();
    let (port, server) = spawn_server(
        "200 OK",
        r#"{"name":"demo","full_name":"my-org/demo","private":false,"fork":false}"#,
    );

    let output = env
        .api_command(format!("127.0.0.1:{port}"))
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
