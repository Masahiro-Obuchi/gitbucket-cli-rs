mod support;

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
}
