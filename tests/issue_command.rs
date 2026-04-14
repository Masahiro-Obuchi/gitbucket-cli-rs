mod support;

use support::gb_cmd::gb_command;

#[test]
fn issue_view_help_mentions_comments_flag_behavior() {
    let output = gb_command()
        .args(["issue", "view", "--help"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(output.status.success());
    assert!(
        stdout.contains("View an issue (use --comments to include comments)"),
        "stdout: {stdout}"
    );
    assert!(stdout.contains("-c, --comments"), "stdout: {stdout}");
    assert!(
        stdout.contains("Include comments in the output"),
        "stdout: {stdout}"
    );
}

#[test]
fn issue_help_mentions_state_values_and_repeatable_metadata_options() {
    let list_output = gb_command()
        .args(["issue", "list", "--help"])
        .output()
        .unwrap();
    let list_stdout = String::from_utf8_lossy(&list_output.stdout);

    assert!(list_output.status.success());
    assert!(
        list_stdout.contains("[possible values: open, closed, all]"),
        "stdout: {list_stdout}"
    );

    let create_output = gb_command()
        .args(["issue", "create", "--help"])
        .output()
        .unwrap();
    let create_stdout = String::from_utf8_lossy(&create_output.stdout);

    assert!(create_output.status.success());
    assert!(
        create_stdout.contains("Label name"),
        "stdout: {create_stdout}"
    );
    assert!(
        create_stdout.contains("repeatable or comma-separated"),
        "stdout: {create_stdout}"
    );
    assert!(
        create_stdout.contains("Assignee username"),
        "stdout: {create_stdout}"
    );
}
