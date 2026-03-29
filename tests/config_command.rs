mod support;

use std::fs;

use serde_json::Value;
use tempfile::tempdir;

use support::gb_cmd::gb_command;

#[test]
fn config_path_prints_config_file_path() {
    let temp = tempdir().unwrap();

    let output = gb_command()
        .current_dir(temp.path())
        .env("GB_CONFIG_DIR", temp.path())
        .args(["config", "path"])
        .output()
        .unwrap();

    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        temp.path().join("config.toml").display().to_string()
    );
}

#[test]
fn config_list_json_shows_hosts_without_exposing_tokens() {
    let temp = tempdir().unwrap();
    fs::write(
        temp.path().join("config.toml"),
        r#"
default_host = "https://gitbucket.example.com/gitbucket"

[hosts."https://gitbucket.example.com/gitbucket"]
token = "secret-token"
user = "alice"
protocol = "https"

[hosts."localhost:8080"]
token = "other-token"
user = "bob"
protocol = "http"
"#,
    )
    .unwrap();

    let output = gb_command()
        .current_dir(temp.path())
        .env("GB_CONFIG_DIR", temp.path())
        .args(["config", "list", "--json"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let payload: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(
        payload["default_host"],
        "https://gitbucket.example.com/gitbucket"
    );
    assert_eq!(
        payload["hosts"]["https://gitbucket.example.com/gitbucket"]["user"],
        "alice"
    );
    assert_eq!(
        payload["hosts"]["https://gitbucket.example.com/gitbucket"]["protocol"],
        "https"
    );
    assert_eq!(
        payload["hosts"]["https://gitbucket.example.com/gitbucket"]["has_token"],
        true
    );
    assert!(payload["hosts"]["https://gitbucket.example.com/gitbucket"]["token"].is_null());
}

#[test]
fn config_list_ignores_runtime_env_overrides() {
    let temp = tempdir().unwrap();
    fs::write(
        temp.path().join("config.toml"),
        r#"
default_host = "stored.example.com"

[hosts."stored.example.com"]
token = "secret-token"
user = "alice"
protocol = "https"
"#,
    )
    .unwrap();

    let output = gb_command()
        .current_dir(temp.path())
        .env("GB_CONFIG_DIR", temp.path())
        .env("GB_HOST", "override.example.com")
        .env("GB_TOKEN", "override-token")
        .env("GB_PROTOCOL", "http")
        .args(["config", "list", "--json"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let payload: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(payload["default_host"], "stored.example.com");
    assert!(payload["hosts"]["stored.example.com"].is_object());
    assert!(payload["hosts"]["override.example.com"].is_null());
}

#[test]
fn config_get_host_field_resolves_equivalent_hostname() {
    let temp = tempdir().unwrap();
    fs::write(
        temp.path().join("config.toml"),
        r#"
[hosts."https://gitbucket.example.com/gitbucket"]
token = "secret-token"
user = "alice"
protocol = "http"
"#,
    )
    .unwrap();

    let output = gb_command()
        .current_dir(temp.path())
        .env("GB_CONFIG_DIR", temp.path())
        .args([
            "config",
            "get",
            "host",
            "--host",
            "gitbucket.example.com/gitbucket",
            "--field",
            "protocol",
        ])
        .output()
        .unwrap();

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "http");
}

#[test]
fn config_get_default_host_fails_when_unset() {
    let temp = tempdir().unwrap();
    fs::write(
        temp.path().join("config.toml"),
        r#"
[hosts."gitbucket.example.com"]
token = "secret-token"
user = "alice"
protocol = "https"
"#,
    )
    .unwrap();

    let output = gb_command()
        .current_dir(temp.path())
        .env("GB_CONFIG_DIR", temp.path())
        .args(["config", "get", "default-host"])
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("No default host configured."),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn config_set_default_host_uses_matching_saved_key() {
    let temp = tempdir().unwrap();
    fs::write(
        temp.path().join("config.toml"),
        r#"
[hosts."https://gitbucket.example.com/gitbucket"]
token = "secret-token"
user = "alice"
protocol = "https"

[hosts."localhost:8080"]
token = "other-token"
user = "bob"
protocol = "http"
"#,
    )
    .unwrap();

    let output = gb_command()
        .current_dir(temp.path())
        .env("GB_CONFIG_DIR", temp.path())
        .args([
            "config",
            "set",
            "default-host",
            "gitbucket.example.com/gitbucket",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let config = fs::read_to_string(temp.path().join("config.toml")).unwrap();
    assert!(config.contains("default_host = \"https://gitbucket.example.com/gitbucket\""));
}

#[test]
fn config_set_host_updates_fields_and_default() {
    let temp = tempdir().unwrap();
    fs::write(
        temp.path().join("config.toml"),
        r#"
[hosts."https://gitbucket.example.com/gitbucket"]
token = "secret-token"
user = "alice"
protocol = "https"
"#,
    )
    .unwrap();

    let output = gb_command()
        .current_dir(temp.path())
        .env("GB_CONFIG_DIR", temp.path())
        .args([
            "config",
            "set",
            "host",
            "--host",
            "gitbucket.example.com/gitbucket",
            "--user",
            "carol",
            "--protocol",
            "http",
            "--default",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let config = fs::read_to_string(temp.path().join("config.toml")).unwrap();
    assert!(config.contains("default_host = \"https://gitbucket.example.com/gitbucket\""));
    assert!(config.contains("user = \"carol\""));
    assert!(config.contains("protocol = \"http\""));
}

#[test]
fn config_set_host_rejects_invalid_protocol() {
    let temp = tempdir().unwrap();
    fs::write(
        temp.path().join("config.toml"),
        r#"
[hosts."https://gitbucket.example.com/gitbucket"]
token = "secret-token"
user = "alice"
protocol = "https"
"#,
    )
    .unwrap();

    let output = gb_command()
        .current_dir(temp.path())
        .env("GB_CONFIG_DIR", temp.path())
        .args([
            "config",
            "set",
            "host",
            "--host",
            "gitbucket.example.com/gitbucket",
            "--protocol",
            "ssh",
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("Protocol must be `http` or `https`."),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn config_set_host_requires_an_explicit_change() {
    let temp = tempdir().unwrap();
    fs::write(
        temp.path().join("config.toml"),
        r#"
[hosts."https://gitbucket.example.com/gitbucket"]
token = "secret-token"
user = "alice"
protocol = "https"
"#,
    )
    .unwrap();

    let output = gb_command()
        .current_dir(temp.path())
        .env("GB_CONFIG_DIR", temp.path())
        .args([
            "config",
            "set",
            "host",
            "--host",
            "gitbucket.example.com/gitbucket",
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .contains("Nothing to update. Specify --user, --protocol, or --default."),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn config_unset_default_host_clears_value() {
    let temp = tempdir().unwrap();
    fs::write(
        temp.path().join("config.toml"),
        r#"
default_host = "https://gitbucket.example.com/gitbucket"

[hosts."https://gitbucket.example.com/gitbucket"]
token = "secret-token"
user = "alice"
protocol = "https"
"#,
    )
    .unwrap();

    let output = gb_command()
        .current_dir(temp.path())
        .env("GB_CONFIG_DIR", temp.path())
        .args(["config", "unset", "default-host"])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let config = fs::read_to_string(temp.path().join("config.toml")).unwrap();
    assert!(!config.contains("default_host"));
}
