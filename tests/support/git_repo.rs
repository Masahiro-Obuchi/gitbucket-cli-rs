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

pub fn init_bare_repo(path: &Path) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    run_git(Path::new("."), &["init", "--bare", path.to_str().unwrap()]);
}

pub fn init_work_repo(path: &Path) {
    std::fs::create_dir_all(path).unwrap();
    run_git(path, &["init"]);
    configure_test_user(path);
}

pub fn configure_test_user(path: &Path) {
    run_git(path, &["config", "user.name", "Test User"]);
    run_git(path, &["config", "user.email", "test@example.com"]);
}

pub fn commit_readme(path: &Path, content: &str, message: &str) {
    std::fs::write(path.join("README.md"), content).unwrap();
    run_git(path, &["add", "README.md"]);
    run_git(path, &["commit", "-m", message]);
}

pub fn commit_tracked_readme(path: &Path, content: &str, message: &str) {
    std::fs::write(path.join("README.md"), content).unwrap();
    run_git(path, &["commit", "-am", message]);
}

pub fn push_main(work: &Path, remote: &Path) {
    run_git(work, &["branch", "-M", "main"]);
    run_git(work, &["remote", "add", "origin", remote.to_str().unwrap()]);
    run_git(work, &["push", "origin", "main"]);
}

pub fn clone_repo(current_dir: &Path, remote: &Path, destination: &Path) {
    run_git(
        current_dir,
        &[
            "clone",
            remote.to_str().unwrap(),
            destination.to_str().unwrap(),
        ],
    );
}

pub fn clone_branch(current_dir: &Path, remote: &Path, branch: &str, destination: &Path) {
    run_git(
        current_dir,
        &[
            "clone",
            "--branch",
            branch,
            remote.to_str().unwrap(),
            destination.to_str().unwrap(),
        ],
    );
}
