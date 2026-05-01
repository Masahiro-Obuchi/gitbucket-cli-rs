use std::path::{Path, PathBuf};
use std::thread;

use crate::support::git_repo::{
    clone_branch, commit_readme, commit_tracked_readme, configure_test_user, init_bare_repo,
    init_work_repo, push_main, run_git,
};
use crate::support::mock_http::{spawn_server, CapturedRequest};

pub fn serve_json_once(
    expected_request_line: &str,
    expected_auth: &str,
    body: &str,
) -> (u16, thread::JoinHandle<CapturedRequest>) {
    let expected_request_line = expected_request_line.to_string();
    let expected_auth = expected_auth.to_ascii_lowercase();
    let (port, server) = spawn_server("200 OK", body);

    let handle = thread::spawn(move || {
        let request = server.join().unwrap();
        let request_line = format!("{} {} HTTP/1.1", request.method, request.target);
        assert_eq!(request_line, expected_request_line);
        let auth = request
            .headers
            .get("authorization")
            .map(|value| format!("authorization: {}", value).to_ascii_lowercase())
            .unwrap_or_default();
        assert!(auth.contains(&expected_auth));
        request
    });

    (port, handle)
}

pub struct PrRemoteFixture {
    pub local_repo: PathBuf,
}

pub fn setup_pr_remote_fixture(
    temp_root: &Path,
    head_branch: &str,
    head_readme: &str,
    head_commit_message: &str,
    reset_head_branch: bool,
) -> PrRemoteFixture {
    let hosting_root = temp_root.join("hosting");
    let base_bare = hosting_root.join("alice").join("base.git");
    let head_bare = hosting_root.join("bob").join("head.git");
    init_bare_repo(&base_bare);
    init_bare_repo(&head_bare);

    let repos_dir = temp_root.join("repos");
    std::fs::create_dir_all(&repos_dir).unwrap();
    let base_work = repos_dir.join("base-work");
    init_work_repo(&base_work);
    commit_readme(&base_work, "base\n", "base");
    push_main(&base_work, &base_bare);

    let head_work = repos_dir.join("head-work");
    clone_branch(temp_root, &base_bare, "main", &head_work);
    configure_test_user(&head_work);
    let checkout_flag = if reset_head_branch { "-B" } else { "-b" };
    run_git(&head_work, &["checkout", checkout_flag, head_branch]);
    commit_tracked_readme(&head_work, head_readme, head_commit_message);
    run_git(
        &head_work,
        &["remote", "add", "fork", head_bare.to_str().unwrap()],
    );
    run_git(&head_work, &["push", "fork", head_branch]);

    let local_repo = temp_root.join("local-repo");
    init_work_repo(&local_repo);
    run_git(
        &local_repo,
        &[
            "config",
            &format!("url.file://{}/.insteadOf", hosting_root.display()),
            "https://gitbucket.example.com/",
        ],
    );
    run_git(
        &local_repo,
        &[
            "remote",
            "add",
            "upstream",
            "https://gitbucket.example.com/alice/base.git",
        ],
    );
    run_git(
        &local_repo,
        &[
            "remote",
            "add",
            "fork",
            "https://gitbucket.example.com/bob/head.git",
        ],
    );

    PrRemoteFixture { local_repo }
}
