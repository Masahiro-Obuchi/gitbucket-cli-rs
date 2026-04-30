mod support;

use serde_json::Value;
use support::gb_cmd::GbTestEnv;
use support::mock_http::spawn_server;

#[test]
fn label_list_prints_json() {
    let env = GbTestEnv::new();
    let (port, server) = spawn_server(
        "200 OK",
        r#"[{"name":"bug","color":"fc2929","description":"Broken behavior"}]"#,
    );

    let output = env
        .repo_api_command(format!("127.0.0.1:{port}"), "alice/project")
        .args(["label", "list", "--json"])
        .output()
        .unwrap();

    let request = server.join().unwrap();

    assert!(output.status.success());
    assert_eq!(request.method, "GET");
    assert_eq!(request.target, "/api/v3/repos/alice/project/labels");

    let payload: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(payload[0]["name"], "bug");
    assert_eq!(payload[0]["description"], "Broken behavior");
}

#[test]
fn label_view_prints_details() {
    let env = GbTestEnv::new();
    let (port, server) = spawn_server(
        "200 OK",
        r#"{"name":"bug","color":"fc2929","description":"Broken behavior","url":"http://example.test/api/v3/repos/alice/project/labels/bug"}"#,
    );

    let output = env
        .repo_api_command(format!("127.0.0.1:{port}"), "alice/project")
        .args(["label", "view", "bug"])
        .output()
        .unwrap();

    let request = server.join().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(output.status.success());
    assert_eq!(request.method, "GET");
    assert_eq!(request.target, "/api/v3/repos/alice/project/labels/bug");
    assert!(stdout.contains("bug"), "stdout: {stdout}");
    assert!(stdout.contains("Color: #fc2929"), "stdout: {stdout}");
    assert!(
        stdout.contains("Description: Broken behavior"),
        "stdout: {stdout}"
    );
}

#[test]
fn label_view_url_encodes_label_name() {
    let env = GbTestEnv::new();
    let (port, server) = spawn_server(
        "200 OK",
        r#"{"name":"needs review","color":"fc2929","description":"Broken behavior"}"#,
    );

    let output = env
        .repo_api_command(format!("127.0.0.1:{port}"), "alice/project")
        .args(["label", "view", "needs review"])
        .output()
        .unwrap();

    let request = server.join().unwrap();

    assert!(output.status.success());
    assert_eq!(request.method, "GET");
    assert_eq!(
        request.target,
        "/api/v3/repos/alice/project/labels/needs%20review"
    );
}

#[test]
fn label_create_sends_expected_payload() {
    let env = GbTestEnv::new();
    let (port, server) = spawn_server(
        "201 Created",
        r#"{"name":"needs-review","color":"abcdef","description":"Needs extra review"}"#,
    );

    let output = env
        .repo_api_command(format!("127.0.0.1:{port}"), "alice/project")
        .args([
            "label",
            "create",
            "needs-review",
            "--color",
            "#ABCDEF",
            "--description",
            "Needs extra review",
        ])
        .output()
        .unwrap();

    let request = server.join().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(request.method, "POST");
    assert_eq!(request.target, "/api/v3/repos/alice/project/labels");
    let body: Value = serde_json::from_str(&request.body).unwrap();
    assert_eq!(body["name"], "needs-review");
    assert_eq!(body["color"], "abcdef");
    assert_eq!(body["description"], "Needs extra review");
    assert!(
        stdout.contains("✓ Created label needs-review"),
        "stdout: {stdout}"
    );
}

#[test]
fn label_create_rejects_invalid_color_before_api_call() {
    let env = GbTestEnv::new();

    let output = env
        .repo_api_command("127.0.0.1:9", "alice/project")
        .args(["label", "create", "bug", "--color", "zzz"])
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .contains("Invalid label color 'zzz'. Expected a 6-digit hex value like ff0000."),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn label_delete_sends_delete_request_when_yes_is_used() {
    let env = GbTestEnv::new();
    let (port, server) = spawn_server("204 No Content", "");

    let output = env
        .repo_api_command(format!("127.0.0.1:{port}"), "alice/project")
        .args(["label", "delete", "bug", "--yes"])
        .output()
        .unwrap();

    let request = server.join().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(request.method, "DELETE");
    assert_eq!(request.target, "/api/v3/repos/alice/project/labels/bug");
    assert!(stdout.contains("✓ Deleted label bug"), "stdout: {stdout}");
}
