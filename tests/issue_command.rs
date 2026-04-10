mod support;

use support::gb_cmd::gb_command;

#[test]
fn issue_view_help_mentions_comments_flag_behavior() {
    let output = gb_command().args(["issue", "view", "--help"]).output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(output.status.success());
    assert!(
        stdout.contains("View an issue (use --comments to include comments)"),
        "stdout: {stdout}"
    );
    assert!(stdout.contains("--comments"), "stdout: {stdout}");
    assert!(
        stdout.contains("Include comments in the output"),
        "stdout: {stdout}"
    );
}
