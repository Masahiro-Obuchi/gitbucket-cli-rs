use super::support::*;
use serde_json::Value;
use tempfile::tempdir;

#[test]
#[ignore = "requires a Docker-backed GitBucket instance bootstrapped via scripts/e2e/bootstrap.sh"]
fn e2e_web_auth_failures_leave_issue_unchanged_and_report_no_write() {
    let temp = tempdir().unwrap();
    login(temp.path());
    let number = create_live_issue(
        temp.path(),
        "web authentication preflight",
        "unchanged body",
    )
    .to_string();
    for wrong_actor in [false, true] {
        let mut command = gb_command();
        command
            .current_dir(temp.path())
            .args(["--json-errors", "issue", "close", &number]);
        for (key, value) in e2e_env(temp.path()) {
            command.env(key, value);
        }
        if wrong_actor {
            command.env("GB_USER", "root");
        } else {
            command.env("GB_PASSWORD", "invalid-e2e-password");
        }
        let output = command.output().unwrap();
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
        let error: Value = serde_json::from_slice(&output.stderr).unwrap();
        assert_eq!(error["error"]["code"], "auth_error");
        assert_eq!(error["error"]["operations"][0]["status"], "not_attempted");
        let actual: Value = serde_json::from_str(&gb_output_with_env(
            temp.path(),
            temp.path(),
            &["issue", "view", &number, "--json"],
        ))
        .unwrap();
        assert_eq!(actual["state"], "open");
        assert_eq!(actual["body"], "unchanged body");
    }
}

#[test]
#[ignore = "requires a Docker-backed GitBucket instance bootstrapped via scripts/e2e/bootstrap.sh"]
fn e2e_explicit_web_user_overrides_stale_git_user_and_names_the_actor() {
    let temp = tempdir().unwrap();
    login(temp.path());
    let number = create_live_issue(temp.path(), "explicit Web actor", "body").to_string();
    let user = required_env("GB_E2E_USER");
    let mut command = gb_command();
    command
        .current_dir(temp.path())
        .args(["issue", "close", &number]);
    for (key, value) in e2e_env(temp.path()) {
        command.env(key, value);
    }
    command
        .env("GB_USER", "stale-git-user")
        .env("GB_WEB_USER", &user);
    let output = command.output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stderr)
        .contains(&format!("Web actor {user}; REST actor {user}")));
    let actual: Value = serde_json::from_str(&gb_output_with_env(
        temp.path(),
        temp.path(),
        &["issue", "view", &number, "--json"],
    ))
    .unwrap();
    assert_eq!(actual["state"], "closed");
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
fn e2e_issue_label_preflight_and_assignee_fallback_keep_label_links() {
    let temp = tempdir().unwrap();
    let user = required_env("GB_E2E_USER");

    login(temp.path());

    let issue_number = create_live_issue(
        temp.path(),
        "issue edit unsupported fields",
        "body for unsupported field checks",
    );

    let label_name = format!("label-preflight-{}", unique_suffix());
    let path = format!(
        "repos/{}/issues/{issue_number}",
        required_env("GB_E2E_REPO")
    );
    let original: Value = serde_json::from_str(&gb_output_with_env(
        temp.path(),
        temp.path(),
        &["api", &path],
    ))
    .unwrap();
    let mut label_command = gb_command();
    label_command.current_dir(temp.path()).args([
        "issue",
        "edit",
        &issue_number.to_string(),
        "--title",
        "must-not-apply",
        "--add-label",
        &label_name,
    ]);
    for (key, value) in e2e_env(temp.path()) {
        label_command.env(key, value);
    }
    label_command.env_remove("GB_PASSWORD");
    let label_stderr = run_and_assert_failure(&mut label_command);
    assert!(
        label_stderr.contains("additions require an existing repository label"),
        "{label_stderr}"
    );
    let unchanged: Value = serde_json::from_str(&gb_output_with_env(
        temp.path(),
        temp.path(),
        &["api", &path],
    ))
    .unwrap();
    assert_eq!(unchanged, original);
    let labels: Value = serde_json::from_str(&gb_output_with_env(
        temp.path(),
        temp.path(),
        &["label", "list", "--json"],
    ))
    .unwrap();
    assert!(!labels
        .as_array()
        .unwrap()
        .iter()
        .any(|label| label["name"] == label_name));
    gb_output_with_env(
        temp.path(),
        temp.path(),
        &["label", "create", &label_name, "--color", "123abc"],
    );
    let mut label_command = gb_command();
    label_command.current_dir(temp.path()).args([
        "issue",
        "edit",
        &issue_number.to_string(),
        "--add-label",
        &label_name,
    ]);
    for (key, value) in e2e_env(temp.path()) {
        label_command.env(key, value);
    }
    label_command.env_remove("GB_PASSWORD");
    run_and_assert_success(&mut label_command);

    let assignee_stdout = gb_output_with_env(
        temp.path(),
        temp.path(),
        &[
            "issue",
            "edit",
            &issue_number.to_string(),
            "--add-assignee",
            &user,
        ],
    );
    assert!(
        assignee_stdout.contains(&format!("Updated issue #{issue_number}:")),
        "stdout: {assignee_stdout}"
    );
    let stored: Value = serde_json::from_str(&gb_output_with_env(
        temp.path(),
        temp.path(),
        &["api", &path],
    ))
    .unwrap();
    assert!(stored["labels"]
        .as_array()
        .unwrap()
        .iter()
        .any(|label| label["name"] == label_name));
    assert!(stored["assignees"]
        .as_array()
        .unwrap()
        .iter()
        .any(|assignee| assignee["login"] == user));
    gb_output_with_env(
        temp.path(),
        temp.path(),
        &["label", "delete", &label_name, "--yes"],
    );
}
