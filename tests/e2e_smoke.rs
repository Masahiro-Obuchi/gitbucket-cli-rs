use assert_cmd::cargo::CommandCargoExt;
use serde_json::Value;
use std::path::Path;
use tempfile::tempdir;
use url::Url;

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

fn run_and_assert_failure(command: &mut std::process::Command) -> String {
    let output = command.output().unwrap();

    assert!(
        !output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    String::from_utf8_lossy(&output.stderr).into_owned()
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

fn create_live_issue(temp: &Path, title: &str, body: &str) -> u64 {
    let stdout = gb_output_with_env(temp, temp, &["issue", "create", "-t", title, "-b", body]);
    parse_issue_number(&stdout)
}

fn add_issue_comment(temp: &Path, number: u64, body: &str) -> String {
    gb_output_with_env(
        temp,
        temp,
        &["issue", "comment", &number.to_string(), "-b", body],
    )
}

fn add_pr_comment(temp: &Path, number: u64, body: &str) -> String {
    gb_output_with_env(
        temp,
        temp,
        &["pr", "comment", &number.to_string(), "-b", body],
    )
}

fn parse_issue_number(stdout: &str) -> u64 {
    let number = stdout
        .split('#')
        .nth(1)
        .and_then(|rest| rest.split(':').next())
        .and_then(|value| value.parse::<u64>().ok());
    number.unwrap_or_else(|| panic!("failed to parse issue number from stdout: {stdout}"))
}

fn parse_pr_number(stdout: &str) -> u64 {
    let number = stdout
        .split('#')
        .nth(1)
        .and_then(|rest| rest.split(':').next())
        .and_then(|value| value.parse::<u64>().ok());
    number.unwrap_or_else(|| panic!("failed to parse PR number from stdout: {stdout}"))
}

fn parse_milestone_number(list_stdout: &str, title: &str) -> u64 {
    let milestones: Value = serde_json::from_str(list_stdout).unwrap();
    milestones
        .as_array()
        .unwrap()
        .iter()
        .find(|milestone| milestone["title"] == title)
        .and_then(|milestone| milestone["number"].as_u64())
        .unwrap_or_else(|| panic!("failed to find milestone '{title}' in output: {list_stdout}"))
}

fn unique_suffix() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis()
}

fn run_git(dir: &Path, args: &[&str]) {
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

fn try_git(dir: &Path, args: &[&str]) -> bool {
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

fn git_output(dir: &Path, args: &[&str]) -> String {
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

fn gb_output_with_env(temp: &Path, current_dir: &Path, args: &[&str]) -> String {
    let mut command = gb_command();
    command.current_dir(current_dir).args(args);
    for (key, value) in e2e_env(temp) {
        command.env(key, value);
    }
    run_and_assert_success(&mut command)
}

fn authenticated_clone_url(temp: &Path, repo: &str) -> String {
    let stdout = gb_output_with_env(temp, temp, &["api", &format!("repos/{repo}")]);
    let payload: Value = serde_json::from_str(&stdout).unwrap();
    let clone_url = payload["clone_url"]
        .as_str()
        .unwrap_or_else(|| panic!("missing clone_url in payload: {stdout}"));
    let user = required_env("GB_E2E_USER");
    let password = required_env("GB_E2E_PASSWORD");

    let mut url = Url::parse(clone_url).unwrap();
    url.set_username(&user).unwrap();
    url.set_password(Some(&password)).unwrap();
    url.to_string()
}

fn clone_repo_to(temp: &Path, repo: &str, destination: &Path) {
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

fn configure_git_identity(repo_dir: &Path) {
    run_git(repo_dir, &["config", "user.name", "GB E2E"]);
    run_git(repo_dir, &["config", "user.email", "gb-e2e@example.test"]);
}

fn ensure_remote_main(repo_dir: &Path) {
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

fn create_live_pr_fixture(temp: &Path, branch_prefix: &str) -> (u64, String, String) {
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

    let issue_number = create_live_issue(temp.path(), "e2e issue", "body");

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
fn e2e_issue_comment_and_view_comments_against_live_instance() {
    let temp = tempdir().unwrap();
    let comment_body = format!("issue comment body {}", unique_suffix());

    login(temp.path());

    let issue_number = create_live_issue(temp.path(), "issue comment target", "body");
    let comment_stdout = add_issue_comment(temp.path(), issue_number, &comment_body);
    assert!(
        comment_stdout.contains(&format!("Added comment to issue #{issue_number}")),
        "stdout: {comment_stdout}"
    );

    let view_without_comments = gb_output_with_env(
        temp.path(),
        temp.path(),
        &["issue", "view", &issue_number.to_string()],
    );
    assert!(
        !view_without_comments.contains("--- Comments ---"),
        "stdout: {view_without_comments}"
    );
    assert!(
        !view_without_comments.contains(&comment_body),
        "stdout: {view_without_comments}"
    );

    let view_with_comments = gb_output_with_env(
        temp.path(),
        temp.path(),
        &["issue", "view", &issue_number.to_string(), "--comments"],
    );
    assert!(
        view_with_comments.contains("--- Comments ---"),
        "stdout: {view_with_comments}"
    );
    assert!(
        view_with_comments.contains(&comment_body),
        "stdout: {view_with_comments}"
    );
    assert!(
        view_with_comments.contains(&required_env("GB_E2E_USER")),
        "stdout: {view_with_comments}"
    );
}

#[test]
#[ignore = "requires a Docker-backed GitBucket instance bootstrapped via scripts/e2e/bootstrap.sh"]
fn e2e_issue_edit_updates_metadata_against_live_instance() {
    let temp = tempdir().unwrap();
    let unique_suffix = unique_suffix();
    let milestone_title = format!("e2e-issue-milestone-{unique_suffix}");

    login(temp.path());

    let create_issue_stdout = gb_output_with_env(
        temp.path(),
        temp.path(),
        &[
            "issue",
            "create",
            "-t",
            "issue edit before",
            "-b",
            "old body",
        ],
    );
    let issue_number = parse_issue_number(&create_issue_stdout);

    let milestone_stdout = gb_output_with_env(
        temp.path(),
        temp.path(),
        &["milestone", "create", &milestone_title],
    );
    assert!(
        milestone_stdout.contains(&milestone_title),
        "stdout: {milestone_stdout}"
    );

    let list_milestones_stdout = gb_output_with_env(
        temp.path(),
        temp.path(),
        &["milestone", "list", "--state", "all", "--json"],
    );
    let milestone_number = parse_milestone_number(&list_milestones_stdout, &milestone_title);

    let edit_stdout = gb_output_with_env(
        temp.path(),
        temp.path(),
        &[
            "issue",
            "edit",
            &issue_number.to_string(),
            "--title",
            "issue edit after",
            "--body",
            "new body",
            "--milestone",
            &milestone_number.to_string(),
            "--state",
            "closed",
        ],
    );
    assert!(
        edit_stdout.contains(&format!("Updated issue #{issue_number}: issue edit after")),
        "stdout: {edit_stdout}"
    );

    let view_stdout = gb_output_with_env(
        temp.path(),
        temp.path(),
        &["issue", "view", &issue_number.to_string()],
    );
    assert!(
        view_stdout.contains("issue edit after"),
        "stdout: {view_stdout}"
    );
    assert!(view_stdout.contains("CLOSED"), "stdout: {view_stdout}");
    assert!(view_stdout.contains("new body"), "stdout: {view_stdout}");
    assert!(
        view_stdout.contains(&format!(
            "Milestone: {milestone_title} (#{milestone_number})"
        )),
        "stdout: {view_stdout}"
    );

    let clear_stdout = gb_output_with_env(
        temp.path(),
        temp.path(),
        &[
            "issue",
            "edit",
            &issue_number.to_string(),
            "--remove-milestone",
            "--state",
            "open",
        ],
    );
    assert!(
        clear_stdout.contains(&format!("Updated issue #{issue_number}: issue edit after")),
        "stdout: {clear_stdout}"
    );

    let view_after_clear = gb_output_with_env(
        temp.path(),
        temp.path(),
        &["issue", "view", &issue_number.to_string()],
    );
    assert!(
        !view_after_clear.contains("Milestone:"),
        "stdout: {view_after_clear}"
    );
}

#[test]
#[ignore = "requires a Docker-backed GitBucket instance bootstrapped via scripts/e2e/bootstrap.sh"]
fn e2e_issue_edit_label_and_assignee_constraints_against_live_instance() {
    let temp = tempdir().unwrap();
    let user = required_env("GB_E2E_USER");

    login(temp.path());

    let issue_number = create_live_issue(
        temp.path(),
        "issue edit unsupported fields",
        "body for unsupported field checks",
    );

    let mut label_command = gb_command();
    label_command.current_dir(temp.path()).args([
        "issue",
        "edit",
        &issue_number.to_string(),
        "--add-label",
        "urgent",
    ]);
    for (key, value) in e2e_env(temp.path()) {
        label_command.env(key, value);
    }
    let label_stderr = run_and_assert_failure(&mut label_command);
    assert!(
        label_stderr.contains(
            "does not support editing issue labels or assignees through the web fallback"
        ),
        "stderr: {label_stderr}"
    );

    let mut assignee_command = gb_command();
    assignee_command.current_dir(temp.path()).args([
        "issue",
        "edit",
        &issue_number.to_string(),
        "--add-assignee",
        &user,
    ]);
    for (key, value) in e2e_env(temp.path()) {
        assignee_command.env(key, value);
    }
    let assignee_stderr = run_and_assert_failure(&mut assignee_command);
    assert!(
        assignee_stderr.contains(
            "does not support editing issue labels or assignees through the web fallback"
        ),
        "stderr: {assignee_stderr}"
    );
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

#[test]
#[ignore = "requires a Docker-backed GitBucket instance bootstrapped via scripts/e2e/bootstrap.sh"]
fn e2e_label_list_create_and_delete_against_live_instance() {
    let temp = tempdir().unwrap();
    let unique_suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis();
    let label_name = format!("e2e-label-{}-{unique_suffix}", std::process::id());

    login(temp.path());

    let mut list_before = gb_command();
    list_before
        .current_dir(temp.path())
        .args(["label", "list", "--json"]);
    for (key, value) in e2e_env(temp.path()) {
        list_before.env(key, value);
    }
    let list_before_stdout = run_and_assert_success(&mut list_before);
    let labels_before: Value = serde_json::from_str(&list_before_stdout).unwrap();
    assert!(
        labels_before.is_array(),
        "label output was not a JSON array: {labels_before}"
    );

    let mut create_command = gb_command();
    create_command.current_dir(temp.path()).args([
        "label",
        "create",
        &label_name,
        "--color",
        "123abc",
        "--description",
        "Created by E2E",
    ]);
    for (key, value) in e2e_env(temp.path()) {
        create_command.env(key, value);
    }
    let create_stdout = run_and_assert_success(&mut create_command);
    assert!(
        create_stdout.contains(&label_name),
        "stdout: {create_stdout}"
    );

    let mut list_after_create = gb_command();
    list_after_create
        .current_dir(temp.path())
        .args(["label", "list", "--json"]);
    for (key, value) in e2e_env(temp.path()) {
        list_after_create.env(key, value);
    }
    let list_after_create_stdout = run_and_assert_success(&mut list_after_create);
    let labels_after_create: Value = serde_json::from_str(&list_after_create_stdout).unwrap();
    assert!(
        labels_after_create
            .as_array()
            .unwrap()
            .iter()
            .any(|label| label["name"] == label_name),
        "stdout: {list_after_create_stdout}"
    );

    let mut delete_command = gb_command();
    delete_command
        .current_dir(temp.path())
        .args(["label", "delete", &label_name, "--yes"]);
    for (key, value) in e2e_env(temp.path()) {
        delete_command.env(key, value);
    }
    let delete_stdout = run_and_assert_success(&mut delete_command);
    assert!(
        delete_stdout.contains(&label_name),
        "stdout: {delete_stdout}"
    );
}

#[test]
#[ignore = "requires a Docker-backed GitBucket instance bootstrapped via scripts/e2e/bootstrap.sh"]
fn e2e_milestone_list_create_edit_and_delete_against_live_instance() {
    let temp = tempdir().unwrap();
    let unique_suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis();
    let title = format!("e2e-milestone-{unique_suffix}");
    let updated_title = format!("{title}-updated");

    login(temp.path());

    let mut create_command = gb_command();
    create_command.current_dir(temp.path()).args([
        "milestone",
        "create",
        &title,
        "--description",
        "Created by E2E",
        "--due-on",
        "2026-04-01",
    ]);
    for (key, value) in e2e_env(temp.path()) {
        create_command.env(key, value);
    }
    let create_stdout = run_and_assert_success(&mut create_command);
    assert!(create_stdout.contains(&title), "stdout: {create_stdout}");

    let mut list_command = gb_command();
    list_command
        .current_dir(temp.path())
        .args(["milestone", "list", "--state", "all", "--json"]);
    for (key, value) in e2e_env(temp.path()) {
        list_command.env(key, value);
    }
    let list_stdout = run_and_assert_success(&mut list_command);
    let milestones: Value = serde_json::from_str(&list_stdout).unwrap();
    let number = milestones
        .as_array()
        .unwrap()
        .iter()
        .find(|milestone| milestone["title"] == title)
        .and_then(|milestone| milestone["number"].as_u64())
        .unwrap_or_else(|| {
            panic!("failed to find created milestone in list output: {list_stdout}")
        });

    let mut edit_command = gb_command();
    edit_command.current_dir(temp.path()).args([
        "milestone",
        "edit",
        &number.to_string(),
        "--title",
        &updated_title,
        "--state",
        "closed",
        "--due-on",
        "2026-04-02",
    ]);
    for (key, value) in e2e_env(temp.path()) {
        edit_command.env(key, value);
    }
    let edit_stdout = run_and_assert_success(&mut edit_command);
    assert!(
        edit_stdout.contains(&updated_title),
        "stdout: {edit_stdout}"
    );

    let mut view_command = gb_command();
    view_command
        .current_dir(temp.path())
        .args(["milestone", "view", &number.to_string()]);
    for (key, value) in e2e_env(temp.path()) {
        view_command.env(key, value);
    }
    let view_stdout = run_and_assert_success(&mut view_command);
    assert!(
        view_stdout.contains(&updated_title),
        "stdout: {view_stdout}"
    );
    assert!(view_stdout.contains("CLOSED"), "stdout: {view_stdout}");

    let mut delete_command = gb_command();
    delete_command.current_dir(temp.path()).args([
        "milestone",
        "delete",
        &number.to_string(),
        "--yes",
    ]);
    for (key, value) in e2e_env(temp.path()) {
        delete_command.env(key, value);
    }
    let delete_stdout = run_and_assert_success(&mut delete_command);
    assert!(
        delete_stdout.contains(&format!("#{number}")),
        "stdout: {delete_stdout}"
    );
}

#[test]
#[ignore = "requires a Docker-backed GitBucket instance bootstrapped via scripts/e2e/bootstrap.sh"]
fn e2e_pr_create_and_merge_against_live_instance() {
    let temp = tempdir().unwrap();
    let repo = required_env("GB_E2E_REPO");

    login(temp.path());

    let (number, branch, file_name) = create_live_pr_fixture(temp.path(), "e2e-pr-merge");

    let view_before = gb_output_with_env(
        temp.path(),
        temp.path(),
        &["pr", "view", &number.to_string()],
    );
    assert!(view_before.contains(&branch), "stdout: {view_before}");

    let merge_stdout = gb_output_with_env(
        temp.path(),
        temp.path(),
        &["pr", "merge", &number.to_string()],
    );
    assert!(
        merge_stdout.contains(&format!("Merged pull request #{number}")),
        "stdout: {merge_stdout}"
    );

    let view_after = gb_output_with_env(
        temp.path(),
        temp.path(),
        &["pr", "view", &number.to_string()],
    );
    assert!(view_after.contains("MERGED"), "stdout: {view_after}");

    let repo_dir = temp.path().join("verify-merge");
    clone_repo_to(temp.path(), &repo, &repo_dir);
    ensure_remote_main(&repo_dir);
    assert!(
        repo_dir.join(&file_name).exists(),
        "file missing after merge"
    );
}

#[test]
#[ignore = "requires a Docker-backed GitBucket instance bootstrapped via scripts/e2e/bootstrap.sh"]
fn e2e_pr_comment_and_view_comments_against_live_instance() {
    let temp = tempdir().unwrap();
    let comment_body = format!("pr comment body {}", unique_suffix());

    login(temp.path());

    let (number, branch, _) = create_live_pr_fixture(temp.path(), "e2e-pr-comment");
    let comment_stdout = add_pr_comment(temp.path(), number, &comment_body);
    assert!(
        comment_stdout.contains(&format!("Added comment to PR #{number}")),
        "stdout: {comment_stdout}"
    );

    let view_without_comments = gb_output_with_env(
        temp.path(),
        temp.path(),
        &["pr", "view", &number.to_string()],
    );
    assert!(
        view_without_comments.contains(&branch),
        "stdout: {view_without_comments}"
    );
    assert!(
        !view_without_comments.contains("--- Comments ---"),
        "stdout: {view_without_comments}"
    );
    assert!(
        !view_without_comments.contains(&comment_body),
        "stdout: {view_without_comments}"
    );

    let view_with_comments = gb_output_with_env(
        temp.path(),
        temp.path(),
        &["pr", "view", &number.to_string(), "--comments"],
    );
    assert!(
        view_with_comments.contains("--- Comments ---"),
        "stdout: {view_with_comments}"
    );
    assert!(
        view_with_comments.contains(&comment_body),
        "stdout: {view_with_comments}"
    );
    assert!(
        view_with_comments.contains(&required_env("GB_E2E_USER")),
        "stdout: {view_with_comments}"
    );
}

#[test]
#[ignore = "requires a Docker-backed GitBucket instance bootstrapped via scripts/e2e/bootstrap.sh"]
fn e2e_pr_checkout_against_live_instance() {
    let temp = tempdir().unwrap();
    let repo = required_env("GB_E2E_REPO");

    login(temp.path());

    let (number, _, _) = create_live_pr_fixture(temp.path(), "e2e-pr-checkout");
    let repo_dir = temp.path().join("checkout-target");
    clone_repo_to(temp.path(), &repo, &repo_dir);
    ensure_remote_main(&repo_dir);
    let main_before = git_output(&repo_dir, &["rev-parse", "main"]);

    let checkout_stdout = gb_output_with_env(
        temp.path(),
        &repo_dir,
        &["pr", "checkout", &number.to_string()],
    );
    assert!(
        checkout_stdout.contains(&format!("pr-{number}")),
        "stdout: {checkout_stdout}"
    );

    let current_branch = git_output(&repo_dir, &["branch", "--show-current"]);
    assert_eq!(current_branch, format!("pr-{number}"));

    let main_after = git_output(&repo_dir, &["rev-parse", "main"]);
    assert_eq!(
        main_before, main_after,
        "local main branch changed unexpectedly"
    );
}

#[test]
#[ignore = "requires a Docker-backed GitBucket instance bootstrapped via scripts/e2e/bootstrap.sh"]
fn e2e_pr_diff_against_live_instance() {
    let temp = tempdir().unwrap();
    let repo = required_env("GB_E2E_REPO");

    login(temp.path());

    let (number, _, file_name) = create_live_pr_fixture(temp.path(), "e2e-pr-diff");
    let repo_dir = temp.path().join("diff-target");
    clone_repo_to(temp.path(), &repo, &repo_dir);
    ensure_remote_main(&repo_dir);

    let diff_stdout =
        gb_output_with_env(temp.path(), &repo_dir, &["pr", "diff", &number.to_string()]);
    assert!(diff_stdout.contains(&file_name), "stdout: {diff_stdout}");
}

#[test]
#[ignore = "requires a Docker-backed GitBucket instance bootstrapped via scripts/e2e/bootstrap.sh"]
fn e2e_repo_clone_against_live_instance() {
    let temp = tempdir().unwrap();
    let repo = required_env("GB_E2E_REPO");
    let clone_target = temp.path().join("cloned-repo");

    login(temp.path());

    let seed_dir = temp.path().join("seed-for-clone");
    clone_repo_to(temp.path(), &repo, &seed_dir);
    ensure_remote_main(&seed_dir);

    let clone_stdout = gb_output_with_env(
        temp.path(),
        temp.path(),
        &["repo", "clone", &repo, clone_target.to_str().unwrap()],
    );
    assert!(clone_target.join(".git").is_dir(), "repo was not cloned");
    assert!(clone_stdout.is_empty(), "stdout: {clone_stdout}");

    let remote = git_output(&clone_target, &["remote", "get-url", "origin"]);
    assert!(
        remote.contains(&repo),
        "origin remote did not reference repo: {remote}"
    );
    assert!(
        clone_target.join("README.md").exists(),
        "README.md missing after clone"
    );
}

#[test]
#[ignore = "requires a Docker-backed GitBucket instance bootstrapped via scripts/e2e/bootstrap.sh"]
fn e2e_repo_delete_against_live_instance() {
    let temp = tempdir().unwrap();
    let user = required_env("GB_E2E_USER");
    let repo_name = format!("e2e-delete-{}", unique_suffix());
    let full_name = format!("{user}/{repo_name}");

    login(temp.path());

    let create_stdout =
        gb_output_with_env(temp.path(), temp.path(), &["repo", "create", &repo_name]);
    assert!(
        create_stdout.contains(&full_name),
        "stdout: {create_stdout}"
    );

    let delete_stdout = gb_output_with_env(
        temp.path(),
        temp.path(),
        &["repo", "delete", &full_name, "--yes"],
    );
    assert!(
        delete_stdout.contains(&full_name),
        "stdout: {delete_stdout}"
    );

    let mut api_command = gb_command();
    api_command
        .current_dir(temp.path())
        .args(["api", &format!("repos/{full_name}")]);
    for (key, value) in e2e_env(temp.path()) {
        api_command.env(key, value);
    }
    let output = api_command.output().unwrap();
    assert!(
        !output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("404"),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}
