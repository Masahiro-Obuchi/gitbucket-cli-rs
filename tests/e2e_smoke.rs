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

fn run_and_assert_success(command: &mut std::process::Command) -> (String, String) {
    let output = command.output().unwrap();

    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    (
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
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

#[test]
#[ignore = "requires a Docker-backed GitBucket instance bootstrapped via scripts/e2e/bootstrap.sh"]
fn e2e_auth_login_and_status_against_live_instance() {
    let temp = tempdir().unwrap();
    let host = required_env("GB_E2E_HOST");

    login(temp.path());

    let (stdout, _) = run_and_assert_success(
        gb_command()
            .current_dir(temp.path())
            .env("GB_CONFIG_DIR", temp.path())
            .env("NO_COLOR", "1")
            .args(["auth", "status"]),
    );

    assert!(stdout.contains(&host), "stdout: {stdout}");
    assert!(stdout.contains("Logged in as root"), "stdout: {stdout}");
}

#[test]
#[ignore = "requires a Docker-backed GitBucket instance bootstrapped via scripts/e2e/bootstrap.sh"]
fn e2e_repo_view_against_live_instance() {
    let temp = tempdir().unwrap();
    let repo = required_env("GB_E2E_REPO");

    login(temp.path());

    let (stdout, _) = run_and_assert_success(
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
    let repo = required_env("GB_E2E_REPO");

    login(temp.path());

    let issue_output = gb_command()
        .current_dir(temp.path())
        .env("GB_CONFIG_DIR", temp.path())
        .env("GB_REPO", &repo)
        .env("NO_COLOR", "1")
        .args(["issue", "list", "--json"])
        .output()
        .unwrap();

    assert!(
        issue_output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&issue_output.stdout),
        String::from_utf8_lossy(&issue_output.stderr)
    );
    let issues: Value = serde_json::from_slice(&issue_output.stdout).unwrap();
    assert!(
        issues.is_array(),
        "issue output was not a JSON array: {issues}"
    );

    let pr_output = gb_command()
        .current_dir(temp.path())
        .env("GB_CONFIG_DIR", temp.path())
        .env("GB_REPO", &repo)
        .env("NO_COLOR", "1")
        .args(["pr", "list", "--json"])
        .output()
        .unwrap();

    assert!(
        pr_output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&pr_output.stdout),
        String::from_utf8_lossy(&pr_output.stderr)
    );
    let prs: Value = serde_json::from_slice(&pr_output.stdout).unwrap();
    assert!(prs.is_array(), "pr output was not a JSON array: {prs}");
}
