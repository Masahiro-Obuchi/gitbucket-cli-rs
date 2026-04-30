use assert_cmd::cargo::CommandCargoExt;
use serde_json::Value;
use std::path::Path;
use url::Url;

pub(super) fn gb_command() -> std::process::Command {
    std::process::Command::cargo_bin("gb").unwrap()
}

pub(super) fn required_env(name: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| panic!("missing required env var: {name}"))
}

pub(super) fn optional_env(name: &str, default: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| default.to_string())
}

pub(super) fn run_and_assert_success(command: &mut std::process::Command) -> String {
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

pub(super) fn run_and_assert_failure(command: &mut std::process::Command) -> String {
    let output = command.output().unwrap();

    assert!(
        !output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    String::from_utf8_lossy(&output.stderr).into_owned()
}

pub(super) fn login(temp: &std::path::Path) {
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

pub(super) fn e2e_env(temp: &std::path::Path) -> Vec<(&'static str, String)> {
    vec![
        ("GB_CONFIG_DIR", temp.display().to_string()),
        ("GB_REPO", required_env("GB_E2E_REPO")),
        ("GB_USER", required_env("GB_E2E_USER")),
        ("GB_PASSWORD", required_env("GB_E2E_PASSWORD")),
        ("NO_COLOR", "1".into()),
    ]
}

pub(super) fn create_live_issue(temp: &Path, title: &str, body: &str) -> u64 {
    let stdout = gb_output_with_env(temp, temp, &["issue", "create", "-t", title, "-b", body]);
    parse_issue_number(&stdout)
}

pub(super) fn add_issue_comment(temp: &Path, number: u64, body: &str) -> String {
    gb_output_with_env(
        temp,
        temp,
        &["issue", "comment", &number.to_string(), "-b", body],
    )
}

pub(super) fn add_pr_comment(temp: &Path, number: u64, body: &str) -> String {
    gb_output_with_env(
        temp,
        temp,
        &["pr", "comment", &number.to_string(), "-b", body],
    )
}

pub(super) fn parse_issue_number(stdout: &str) -> u64 {
    let number = stdout
        .split('#')
        .nth(1)
        .and_then(|rest| rest.split(':').next())
        .and_then(|value| value.parse::<u64>().ok());
    number.unwrap_or_else(|| panic!("failed to parse issue number from stdout: {stdout}"))
}

pub(super) fn parse_pr_number(stdout: &str) -> u64 {
    let number = stdout
        .split('#')
        .nth(1)
        .and_then(|rest| rest.split(':').next())
        .and_then(|value| value.parse::<u64>().ok());
    number.unwrap_or_else(|| panic!("failed to parse PR number from stdout: {stdout}"))
}

pub(super) fn parse_milestone_number(list_stdout: &str, title: &str) -> u64 {
    let milestones: Value = serde_json::from_str(list_stdout).unwrap();
    milestones
        .as_array()
        .unwrap()
        .iter()
        .find(|milestone| milestone["title"] == title)
        .and_then(|milestone| milestone["number"].as_u64())
        .unwrap_or_else(|| panic!("failed to find milestone '{title}' in output: {list_stdout}"))
}

pub(super) fn unique_suffix() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis()
}

pub(super) fn run_git(dir: &Path, args: &[&str]) {
    let output = std::process::Command::new("git")
        .current_dir(dir)
        .env("GIT_TERMINAL_PROMPT", "0")
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git {:?} failed\nstdout: {}\nstderr: {}",
        args,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

pub(super) fn try_git(dir: &Path, args: &[&str]) -> bool {
    std::process::Command::new("git")
        .current_dir(dir)
        .env("GIT_TERMINAL_PROMPT", "0")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .args(args)
        .status()
        .unwrap()
        .success()
}

pub(super) fn git_output(dir: &Path, args: &[&str]) -> String {
    let output = std::process::Command::new("git")
        .current_dir(dir)
        .env("GIT_TERMINAL_PROMPT", "0")
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git {:?} failed\nstdout: {}\nstderr: {}",
        args,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

pub(super) fn gb_output_with_env(temp: &Path, current_dir: &Path, args: &[&str]) -> String {
    let mut command = gb_command();
    command.current_dir(current_dir).args(args);
    for (key, value) in e2e_env(temp) {
        command.env(key, value);
    }
    run_and_assert_success(&mut command)
}

pub(super) fn authenticated_clone_url(temp: &Path, repo: &str) -> String {
    let stdout = gb_output_with_env(temp, temp, &["api", &format!("repos/{repo}")]);
    let payload: Value = serde_json::from_str(&stdout).unwrap();
    let clone_url = payload["clone_url"]
        .as_str()
        .unwrap_or_else(|| panic!("missing clone_url in payload: {stdout}"));
    let base_url = required_env("GB_E2E_BASE_URL");
    let user = required_env("GB_E2E_USER");
    let password = required_env("GB_E2E_PASSWORD");

    let mut url = Url::parse(&base_url).unwrap();
    let api_url = Url::parse(clone_url).unwrap();
    let base_prefix = url.path().trim_end_matches('/').to_string();
    let api_path = api_url.path();
    let normalized_api_path = if api_path.starts_with('/') {
        api_path.to_string()
    } else {
        format!("/{api_path}")
    };
    let combined_path =
        if base_prefix.is_empty() || normalized_api_path.starts_with(&format!("{base_prefix}/")) {
            normalized_api_path
        } else {
            format!("{base_prefix}{normalized_api_path}")
        };

    url.set_path(&combined_path);
    url.set_query(api_url.query());
    url.set_fragment(api_url.fragment());
    url.set_username(&user).unwrap();
    url.set_password(Some(&password)).unwrap();
    url.to_string()
}

pub(super) fn clone_repo_to(temp: &Path, repo: &str, destination: &Path) {
    let clone_url = authenticated_clone_url(temp, repo);
    let output = std::process::Command::new("git")
        .current_dir(temp)
        .env("GIT_TERMINAL_PROMPT", "0")
        .args(["clone", &clone_url, destination.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git clone failed\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

pub(super) fn configure_git_identity(repo_dir: &Path) {
    run_git(repo_dir, &["config", "user.name", "GB E2E"]);
    run_git(repo_dir, &["config", "user.email", "gb-e2e@example.test"]);
}

pub(super) fn ensure_remote_main(repo_dir: &Path) {
    configure_git_identity(repo_dir);

    if try_git(
        repo_dir,
        &["ls-remote", "--exit-code", "--heads", "origin", "main"],
    ) {
        run_git(
            repo_dir,
            &["fetch", "origin", "main:refs/remotes/origin/main"],
        );
        run_git(
            repo_dir,
            &["checkout", "-B", "main", "refs/remotes/origin/main"],
        );
        return;
    }

    std::fs::write(repo_dir.join("README.md"), "seed\n").unwrap();
    run_git(repo_dir, &["add", "README.md"]);
    run_git(repo_dir, &["commit", "-m", "Seed main branch"]);
    run_git(repo_dir, &["branch", "-M", "main"]);
    if !try_git(repo_dir, &["push", "-u", "origin", "main"]) {
        run_git(
            repo_dir,
            &["fetch", "origin", "main:refs/remotes/origin/main"],
        );
        run_git(
            repo_dir,
            &["checkout", "-B", "main", "refs/remotes/origin/main"],
        );
    }
}

pub(super) fn create_live_pr_fixture(temp: &Path, branch_prefix: &str) -> (u64, String, String) {
    let repo = required_env("GB_E2E_REPO");
    let suffix = unique_suffix();
    let branch = format!("{branch_prefix}-{suffix}");
    let file_name = format!("{branch}.txt");
    let marker = format!("marker-{suffix}");
    let repo_dir = temp.join(format!("work-{branch}"));

    clone_repo_to(temp, &repo, &repo_dir);
    ensure_remote_main(&repo_dir);

    run_git(&repo_dir, &["checkout", "-B", &branch, "main"]);
    std::fs::write(repo_dir.join(&file_name), format!("{marker}\n")).unwrap();
    run_git(&repo_dir, &["add", &file_name]);
    run_git(&repo_dir, &["commit", "-m", &format!("Add {branch}")]);
    run_git(&repo_dir, &["push", "-u", "origin", &branch]);

    let stdout = gb_output_with_env(
        temp,
        &repo_dir,
        &[
            "pr",
            "create",
            "-t",
            &format!("E2E PR {branch}"),
            "-b",
            &format!("Created by E2E for {branch}"),
            "--head",
            &branch,
            "--base",
            "main",
        ],
    );

    (parse_pr_number(&stdout), branch, file_name)
}
