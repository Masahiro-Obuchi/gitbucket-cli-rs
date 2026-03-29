use assert_cmd::cargo::CommandCargoExt;
use serde_json::Value;
use tempfile::tempdir;

fn gb_command() -> std::process::Command {
    std::process::Command::cargo_bin("gb").unwrap()
}

fn required_env(name: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| panic!("missing required env var: {name}"))
}

fn optional_env(name: &str, default: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| default.to_string())
}

fn run_and_assert_success(command: &mut std::process::Command) -> String {
    let output = command.output().unwrap();

    assert!(
        output.status.success(),
        "stdout: {}
stderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn login(temp: &std::path::Path) {
    let host = required_env("GB_E2E_HOST");
    let token = required_env("GB_E2E_TOKEN");
    let protocol = optional_env("GB_E2E_PROTOCOL", "http");

    let _ = run_and_assert_success(
        gb_command()
            .current_dir(temp)
            .env("GB_CONFIG_DIR", temp)
            .env("NO_COLOR", "1")
            .args([
                "auth",
                "login",
                "-H",
                &host,
                "-t",
                &token,
                "--protocol",
                &protocol,
            ]),
    );
}

fn e2e_env(temp: &std::path::Path) -> Vec<(&'static str, String)> {
    vec![
        ("GB_CONFIG_DIR", temp.display().to_string()),
        ("GB_REPO", required_env("GB_E2E_REPO")),
        ("GB_USER", required_env("GB_E2E_USER")),
        ("GB_PASSWORD", required_env("GB_E2E_PASSWORD")),
        ("NO_COLOR", "1".into()),
    ]
}

fn parse_issue_number(stdout: &str) -> u64 {
    let number = stdout
        .split('#')
        .nth(1)
        .and_then(|rest| rest.split(':').next())
        .and_then(|value| value.parse::<u64>().ok());
    number.unwrap_or_else(|| panic!("failed to parse issue number from stdout: {stdout}"))
}

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

#[test]
#[ignore = "requires a Docker-backed GitBucket instance bootstrapped via scripts/e2e/bootstrap.sh"]
fn e2e_repo_view_against_live_instance() {
    let temp = tempdir().unwrap();
    let repo = required_env("GB_E2E_REPO");

    login(temp.path());

    let stdout = run_and_assert_success(
        gb_command()
            .current_dir(temp.path())
            .env("GB_CONFIG_DIR", temp.path())
            .env("NO_COLOR", "1")
            .args(["repo", "view", &repo]),
    );

    assert!(stdout.contains(&repo), "stdout: {stdout}");
    assert!(stdout.contains("Visibility:"), "stdout: {stdout}");
}

#[test]
#[ignore = "requires a Docker-backed GitBucket instance bootstrapped via scripts/e2e/bootstrap.sh"]
fn e2e_issue_and_pr_list_json_against_live_instance() {
    let temp = tempdir().unwrap();

    login(temp.path());

    let mut issue_command = gb_command();
    issue_command
        .current_dir(temp.path())
        .args(["issue", "list", "--json"]);
    for (key, value) in e2e_env(temp.path()) {
        issue_command.env(key, value);
    }
    let issue_output = issue_command.output().unwrap();

    assert!(
        issue_output.status.success(),
        "stdout: {}
stderr: {}",
        String::from_utf8_lossy(&issue_output.stdout),
        String::from_utf8_lossy(&issue_output.stderr)
    );
    let issues: Value = serde_json::from_slice(&issue_output.stdout).unwrap();
    assert!(
        issues.is_array(),
        "issue output was not a JSON array: {issues}"
    );

    let mut pr_command = gb_command();
    pr_command
        .current_dir(temp.path())
        .args(["pr", "list", "--json"]);
    for (key, value) in e2e_env(temp.path()) {
        pr_command.env(key, value);
    }
    let pr_output = pr_command.output().unwrap();

    assert!(
        pr_output.status.success(),
        "stdout: {}
stderr: {}",
        String::from_utf8_lossy(&pr_output.stdout),
        String::from_utf8_lossy(&pr_output.stderr)
    );
    let prs: Value = serde_json::from_slice(&pr_output.stdout).unwrap();
    assert!(prs.is_array(), "pr output was not a JSON array: {prs}");
}

#[test]
#[ignore = "requires a Docker-backed GitBucket instance bootstrapped via scripts/e2e/bootstrap.sh"]
fn e2e_issue_close_and_reopen_against_live_instance() {
    let temp = tempdir().unwrap();

    login(temp.path());

    let mut create_command = gb_command();
    create_command.current_dir(temp.path()).args([
        "issue",
        "create",
        "-t",
        "e2e issue",
        "-b",
        "body",
    ]);
    for (key, value) in e2e_env(temp.path()) {
        create_command.env(key, value);
    }
    let create_stdout = run_and_assert_success(&mut create_command);
    let issue_number = parse_issue_number(&create_stdout);

    let mut close_command = gb_command();
    close_command
        .current_dir(temp.path())
        .args(["issue", "close", &issue_number.to_string()]);
    for (key, value) in e2e_env(temp.path()) {
        close_command.env(key, value);
    }
    let close_stdout = run_and_assert_success(&mut close_command);
    assert!(close_stdout.contains(&format!("Closed issue #{issue_number}")));

    let mut reopen_command = gb_command();
    reopen_command
        .current_dir(temp.path())
        .args(["issue", "reopen", &issue_number.to_string()]);
    for (key, value) in e2e_env(temp.path()) {
        reopen_command.env(key, value);
    }
    let reopen_stdout = run_and_assert_success(&mut reopen_command);
    assert!(reopen_stdout.contains(&format!("Reopened issue #{issue_number}")));
}

#[test]
#[ignore = "requires a Docker-backed GitBucket instance bootstrapped via scripts/e2e/bootstrap.sh"]
fn e2e_repo_fork_against_live_instance() {
    let temp = tempdir().unwrap();
    let fork_source = required_env("GB_E2E_FORK_SOURCE");
    let user = required_env("GB_E2E_USER");

    login(temp.path());

    let mut fork_command = gb_command();
    fork_command
        .current_dir(temp.path())
        .args(["repo", "fork", &fork_source]);
    for (key, value) in e2e_env(temp.path()) {
        fork_command.env(key, value);
    }
    let stdout = run_and_assert_success(&mut fork_command);

    assert!(stdout.contains(&fork_source), "stdout: {stdout}");
    assert!(stdout.contains(&format!("→ {user}/")), "stdout: {stdout}");
}
