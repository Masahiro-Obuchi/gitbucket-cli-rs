use super::support::*;
use serde_json::Value;
use tempfile::tempdir;

#[test]
#[ignore = "requires a Docker-backed GitBucket instance bootstrapped via scripts/e2e/bootstrap.sh"]
fn e2e_auth_login_and_status_against_live_instance() {
    let temp = tempdir().unwrap();
    let host = required_env("GB_E2E_HOST");
    let user = required_env("GB_E2E_USER");

    login(temp.path());

    let stdout = run_and_assert_success(
        gb_command()
            .current_dir(temp.path())
            .env("GB_CONFIG_DIR", temp.path())
            .env("NO_COLOR", "1")
            .args(["auth", "status"]),
    );

    assert!(stdout.contains(&host), "stdout: {stdout}");
    assert!(
        stdout.contains(&format!("Logged in as {user}")),
        "stdout: {stdout}"
    );
}

#[test]
#[ignore = "requires a Docker-backed GitBucket instance bootstrapped via scripts/e2e/bootstrap.sh"]
fn e2e_config_round_trip_against_live_instance() {
    let temp = tempdir().unwrap();
    let host = required_env("GB_E2E_HOST");
    let base_url = required_env("GB_E2E_BASE_URL");
    let user = required_env("GB_E2E_USER");
    let protocol = optional_env("GB_E2E_PROTOCOL", "http");
    let host_alias = format!("{base_url}/api/v3");

    login(temp.path());

    let default_host = run_and_assert_success(
        gb_command()
            .current_dir(temp.path())
            .env("GB_CONFIG_DIR", temp.path())
            .env("NO_COLOR", "1")
            .args(["config", "get", "default-host"]),
    );
    assert_eq!(default_host.trim(), host);

    let host_json = run_and_assert_success(
        gb_command()
            .current_dir(temp.path())
            .env("GB_CONFIG_DIR", temp.path())
            .env("NO_COLOR", "1")
            .args(["config", "get", "host", "--host", &host_alias, "--json"]),
    );
    let host_payload: Value = serde_json::from_str(&host_json).unwrap();
    assert_eq!(host_payload["hostname"], host);
    assert_eq!(host_payload["user"], user);
    assert_eq!(host_payload["protocol"], protocol);
    assert_eq!(host_payload["has_token"], true);

    let unset_stdout = run_and_assert_success(
        gb_command()
            .current_dir(temp.path())
            .env("GB_CONFIG_DIR", temp.path())
            .env("NO_COLOR", "1")
            .args(["config", "unset", "default-host"]),
    );
    assert!(
        unset_stdout.contains("Cleared default host"),
        "stdout: {unset_stdout}"
    );

    let get_after_unset = gb_command()
        .current_dir(temp.path())
        .env("GB_CONFIG_DIR", temp.path())
        .env("NO_COLOR", "1")
        .args(["config", "get", "default-host"])
        .output()
        .unwrap();
    assert!(!get_after_unset.status.success());
    assert!(
        String::from_utf8_lossy(&get_after_unset.stderr).contains("No default host configured."),
        "stderr: {}",
        String::from_utf8_lossy(&get_after_unset.stderr)
    );

    let set_stdout = run_and_assert_success(
        gb_command()
            .current_dir(temp.path())
            .env("GB_CONFIG_DIR", temp.path())
            .env("NO_COLOR", "1")
            .args(["config", "set", "default-host", &host_alias]),
    );
    assert!(set_stdout.contains(&host), "stdout: {set_stdout}");
}

#[test]
#[ignore = "requires a Docker-backed GitBucket instance bootstrapped via scripts/e2e/bootstrap.sh"]
fn e2e_api_user_against_live_instance() {
    let temp = tempdir().unwrap();
    let user = required_env("GB_E2E_USER");

    login(temp.path());

    let stdout = run_and_assert_success(
        gb_command()
            .current_dir(temp.path())
            .env("GB_CONFIG_DIR", temp.path())
            .env("NO_COLOR", "1")
            .args(["api", "/api/v3/user"]),
    );

    let payload: Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(payload["login"], user);
}
