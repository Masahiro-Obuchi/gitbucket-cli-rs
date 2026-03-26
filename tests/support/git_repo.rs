#![allow(dead_code)]
use std::path::Path;
use std::process::Command;

pub fn run_git(dir: &Path, args: &[&str]) {
    let status = Command::new("git")
        .current_dir(dir)
        .args(args)
        .status()
        .unwrap();
    assert!(status.success(), "git {:?} failed", args);
}

pub fn git_output(dir: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .current_dir(dir)
        .args(args)
        .output()
        .unwrap();
    assert!(output.status.success(), "git {:?} failed", args);
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}
