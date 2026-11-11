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
    assert_eq!(default_host.trim(), base_url);

    let host_json = run_and_assert_success(
        gb_command()
            .current_dir(temp.path())
            .env("GB_CONFIG_DIR", temp.path())
            .env("NO_COLOR", "1")
            .args(["config", "get", "host", "--host", &host_alias, "--json"]),
    );
    let host_payload: Value = serde_json::from_str(&host_json).unwrap();
    assert_eq!(host_payload["hostname"], base_url);
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

#[test]
#[ignore = "requires a Docker-backed GitBucket instance bootstrapped via scripts/e2e/bootstrap.sh"]
fn e2e_profile_status_logout_and_migration_preserve_other_scopes() {
    let temp = tempdir().unwrap();
    let target = required_env("GB_E2E_BASE_URL");
    let token = required_env("GB_E2E_TOKEN");
    let user = required_env("GB_E2E_USER");
    login(temp.path());
    let invoke = |args: &[&str]| {
        let mut cmd = gb_command();
        cmd.current_dir(temp.path())
            .env("GB_CONFIG_DIR", temp.path())
            .env("NO_COLOR", "1")
            .env_remove("GB_TOKEN")
            .env_remove("GB_PROFILE")
            .env_remove("GB_HOST")
            .env_remove("GB_PROTOCOL")
            .args(args);
        run_and_assert_success(&mut cmd)
    };
    for profile in ["work", "personal"] {
        invoke(&[
            "auth",
            "login",
            "--profile",
            profile,
            "-H",
            &target,
            "-t",
            &token,
        ]);
    }
    invoke(&["config", "set", "default-profile", "work"]);
    invoke(&[
        "config",
        "set",
        "host",
        "--host",
        &target,
        "--user",
        "stale-hint",
    ]);
    let status: Value = serde_json::from_str(&invoke(&["auth", "status", "--json"])).unwrap();
    assert_eq!(status["effective_actor"]["user"], user);
    assert_eq!(status["effective_actor"]["stored_user"], "stale-hint");
    assert_eq!(status["effective_actor"]["credential_source"], "profile");
    invoke(&["auth", "logout"]);
    let status: Value = serde_json::from_str(&invoke(&["auth", "status", "--json"])).unwrap();
    assert_eq!(status["effective_actor"]["user"], user);
    assert_eq!(status["effective_actor"]["credential_source"], "global");
    let personal: Value = serde_json::from_str(&invoke(&[
        "auth",
        "status",
        "--profile",
        "personal",
        "--json",
    ]))
    .unwrap();
    assert_eq!(personal["effective_actor"]["credential_source"], "profile");
    // Exercise a legacy key using a real valid credential, then roll back the
    // exact original file while keeping the other profiles intact.
    let path = temp.path().join("config.toml");
    let mut config: toml::Value = toml::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    let legacy = target.strip_prefix("http://").unwrap();
    let entry = config["hosts"]
        .as_table_mut()
        .unwrap()
        .remove(&target)
        .unwrap();
    config["hosts"]
        .as_table_mut()
        .unwrap()
        .insert(legacy.to_string(), entry);
    let original = toml::to_string_pretty(&config).unwrap();
    std::fs::write(&path, &original).unwrap();
    let report: Value = serde_json::from_str(&invoke(&["config", "migrate", "--apply"])).unwrap();
    assert_eq!(report["changed"], true);
    let backup = report["backup"].as_str().unwrap();
    let status: Value = serde_json::from_str(&invoke(&["auth", "status", "--json"])).unwrap();
    assert_eq!(status["effective_actor"]["user"], user);
    invoke(&["config", "migrate", "--restore", backup]);
    assert_eq!(std::fs::read_to_string(&path).unwrap(), original);
    let personal: Value = serde_json::from_str(&invoke(&[
        "auth",
        "status",
        "--profile",
        "personal",
        "--json",
    ]))
    .unwrap();
    assert_eq!(personal["effective_actor"]["user"], user);
}
