use crate::support::gb_cmd::GbTestEnv;
use crate::support::git_repo::{commit_readme, init_work_repo, run_git};

#[test]
fn pr_create_fails_cleanly_when_head_is_detached() {
    let env = GbTestEnv::new();
    init_work_repo(env.path());
    commit_readme(env.path(), "hello\n", "initial");
    run_git(env.path(), &["checkout", "--detach", "HEAD"]);

    let output = env
        .repo_api_command("gitbucket.example.com", "alice/project")
        .args([
            "pr",
            "create",
            "-t",
            "Detached PR",
            "-b",
            "body",
            "--base",
            "main",
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Could not determine current branch"),
        "stderr: {stderr}"
    );
}
