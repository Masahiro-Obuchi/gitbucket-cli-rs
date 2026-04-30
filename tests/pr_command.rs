mod support;

use support::gb_cmd::{gb_command, GbTestEnv};

#[test]
fn pr_create_help_mentions_cross_repo_head_syntax() {
    let output = gb_command()
        .args(["pr", "create", "--help"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(output.status.success());
    assert!(
        stdout.contains("OWNER:BRANCH") || stdout.contains("OWNER"),
        "stdout: {stdout}"
    );
    assert!(stdout.contains("--head-owner"), "stdout: {stdout}");
    assert!(stdout.contains("--json"), "stdout: {stdout}");
    assert!(stdout.contains("--detect-existing"), "stdout: {stdout}");
}

#[test]
fn pr_help_mentions_edit_and_comment_edit_last() {
    let pr_output = gb_command().args(["pr", "--help"]).output().unwrap();
    let pr_stdout = String::from_utf8_lossy(&pr_output.stdout);

    assert!(pr_output.status.success());
    assert!(pr_stdout.contains("edit"), "stdout: {pr_stdout}");

    let comment_output = gb_command()
        .args(["pr", "comment", "--help"])
        .output()
        .unwrap();
    let comment_stdout = String::from_utf8_lossy(&comment_output.stdout);

    assert!(comment_output.status.success());
    assert!(
        comment_stdout.contains("--edit-last"),
        "stdout: {comment_stdout}"
    );
    assert!(
        comment_stdout.contains("--json"),
        "stdout: {comment_stdout}"
    );
    assert!(comment_stdout.contains("list"), "stdout: {comment_stdout}");

    let comment_list_output = gb_command()
        .args(["pr", "comment", "list", "--help"])
        .output()
        .unwrap();
    let comment_list_stdout = String::from_utf8_lossy(&comment_list_output.stdout);

    assert!(comment_list_output.status.success());
    assert!(
        comment_list_stdout.contains("--json"),
        "stdout: {comment_list_stdout}"
    );
}

#[test]
fn pr_diff_help_mentions_no_pager() {
    let diff_output = gb_command()
        .args(["pr", "diff", "--help"])
        .output()
        .unwrap();
    let diff_stdout = String::from_utf8_lossy(&diff_output.stdout);
    assert!(diff_output.status.success());
    assert!(diff_stdout.contains("--no-pager"), "stdout: {diff_stdout}");
}

#[test]
fn pr_read_help_mentions_no_pager() {
    for args in [
        ["pr", "list", "--help"].as_slice(),
        ["pr", "view", "--help"].as_slice(),
        ["pr", "comment", "list", "--help"].as_slice(),
    ] {
        let output = gb_command().args(args).output().unwrap();
        let stdout = String::from_utf8_lossy(&output.stdout);

        assert!(output.status.success());
        assert!(stdout.contains("--no-pager"), "stdout: {stdout}");
    }
}

#[test]
fn pr_create_rejects_head_owner_containing_colon() {
    let env = GbTestEnv::new();
    let output = env
        .repo_api_command("127.0.0.1:19999", "alice/project")
        .args([
            "pr",
            "create",
            "--head",
            "feature",
            "--head-owner",
            "bob:team",
            "--base",
            "main",
            "--title",
            "Test PR",
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("--head-owner cannot contain ':'"),
        "stderr: {stderr}"
    );
}

#[test]
fn json_errors_print_structured_failure() {
    let env = GbTestEnv::new();
    let output = env
        .repo_api_command("127.0.0.1:19999", "alice/project")
        .args([
            "--json-errors",
            "pr",
            "create",
            "--head-owner",
            "bad:owner",
            "--head",
            "feature",
            "--title",
            "Demo",
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    let value: serde_json::Value = serde_json::from_str(stderr.trim()).unwrap();
    assert_eq!(value["error"]["code"], "error");
    assert_eq!(value["error"]["exit_code"], 1);
    assert!(
        value["error"]["message"]
            .as_str()
            .unwrap()
            .contains("--head-owner cannot contain ':'"),
        "stderr: {stderr}"
    );
}
