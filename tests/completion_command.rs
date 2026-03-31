mod support;

use support::gb_cmd::gb_command;

#[test]
fn completion_bash_prints_script() {
    let output = gb_command().args(["completion", "bash"]).output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(output.status.success());
    assert!(stdout.contains("_gb()"), "stdout: {stdout}");
    assert!(stdout.contains("complete -F _gb -o bashdefault -o default gb"), "stdout: {stdout}");
}

#[test]
fn completion_zsh_prints_script() {
    let output = gb_command().args(["completion", "zsh"]).output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(output.status.success());
    assert!(stdout.contains("#compdef gb"), "stdout: {stdout}");
    assert!(stdout.contains("_gb"), "stdout: {stdout}");
}

#[test]
fn completion_fish_prints_script() {
    let output = gb_command().args(["completion", "fish"]).output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(output.status.success());
    assert!(stdout.contains("complete -c gb"), "stdout: {stdout}");
}

#[test]
fn completion_powershell_prints_script() {
    let output = gb_command()
        .args(["completion", "powershell"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(output.status.success());
    assert!(stdout.contains("Register-ArgumentCompleter"), "stdout: {stdout}");
}

#[test]
fn completion_help_lists_supported_shells() {
    let output = gb_command().args(["completion", "--help"]).output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(output.status.success());
    assert!(stdout.contains("Generate shell completion scripts"), "stdout: {stdout}");
    assert!(stdout.contains("bash"), "stdout: {stdout}");
    assert!(stdout.contains("zsh"), "stdout: {stdout}");
    assert!(stdout.contains("fish"), "stdout: {stdout}");
    assert!(stdout.contains("powershell"), "stdout: {stdout}");
}
