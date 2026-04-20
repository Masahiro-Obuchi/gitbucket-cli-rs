mod support;

use tempfile::tempdir;

use support::gb_cmd::gb_command;

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
}

#[test]
fn pr_view_and_diff_help_mention_no_pager() {
    let view_output = gb_command()
        .args(["pr", "view", "--help"])
        .output()
        .unwrap();
    let view_stdout = String::from_utf8_lossy(&view_output.stdout);
    assert!(view_output.status.success());
    assert!(view_stdout.contains("--no-pager"), "stdout: {view_stdout}");

    let diff_output = gb_command()
        .args(["pr", "diff", "--help"])
        .output()
        .unwrap();
    let diff_stdout = String::from_utf8_lossy(&diff_output.stdout);
    assert!(diff_output.status.success());
    assert!(diff_stdout.contains("--no-pager"), "stdout: {diff_stdout}");
}

#[test]
fn pr_create_rejects_head_owner_containing_colon() {
    let temp = tempdir().unwrap();
    let output = gb_command()
        .current_dir(temp.path())
        .env("GB_CONFIG_DIR", temp.path())
        .env("GB_HOST", "127.0.0.1:19999")
        .env("GB_REPO", "alice/project")
        .env("GB_TOKEN", "test-token")
        .env("GB_PROTOCOL", "http")
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
