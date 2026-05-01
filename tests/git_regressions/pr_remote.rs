use crate::common::{serve_json_once, setup_pr_remote_fixture};
use crate::support::gb_cmd::GbTestEnv;
use crate::support::git_repo::{commit_readme, git_output, run_git};

#[test]
fn pr_checkout_prefers_matching_remote_when_api_clone_url_is_unusable() {
    let env = GbTestEnv::new();
    let fixture = setup_pr_remote_fixture(
        env.path(),
        "feature/demo",
        "base\nfeature\n",
        "feature",
        false,
    );
    let local_repo = fixture.local_repo;

    let body = concat!(
        "{",
        "\"number\":5,",
        "\"title\":\"Feature\",",
        "\"state\":\"open\",",
        "\"head\":{\"ref\":\"feature/demo\",\"repo\":{\"name\":\"head\",\"full_name\":\"bob/head\",\"private\":true,\"clone_url\":\"git@gitbucket.example.com:bob/head.git\"}},",
        "\"base\":{\"ref\":\"main\",\"repo\":{\"name\":\"base\",\"full_name\":\"alice/base\",\"private\":false,\"clone_url\":\"git@gitbucket.example.com:alice/base.git\"}}",
        "}"
    )
    .to_string();
    let (port, server) = serve_json_once(
        "GET /api/v3/repos/alice/project/pulls/5 HTTP/1.1",
        "authorization: token test-token",
        &body,
    );

    let output = env
        .repo_api_command(format!("127.0.0.1:{port}"), "alice/project")
        .current_dir(&local_repo)
        .args(["pr", "checkout", "5"])
        .output()
        .unwrap();

    server.join().unwrap();

    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        git_output(&local_repo, &["branch", "--show-current"]),
        "pr-5"
    );
    let content = std::fs::read_to_string(local_repo.join("README.md")).unwrap();
    assert!(content.contains("feature"), "README content: {content}");
}

#[test]
fn pr_checkout_does_not_overwrite_local_main_when_pr_branch_is_named_main() {
    let env = GbTestEnv::new();
    let fixture = setup_pr_remote_fixture(env.path(), "main", "base\npr-main\n", "pr main", true);
    let local_repo = fixture.local_repo;
    commit_readme(&local_repo, "local-main\n", "local main");
    run_git(&local_repo, &["branch", "-M", "main"]);
    let local_main_before = git_output(&local_repo, &["rev-parse", "main"]);

    let body = concat!(
        "{",
        "\"number\":7,",
        "\"title\":\"Main branch PR\",",
        "\"state\":\"open\",",
        "\"head\":{\"ref\":\"main\",\"repo\":{\"name\":\"head\",\"full_name\":\"bob/head\",\"private\":true,\"clone_url\":\"git@gitbucket.example.com:bob/head.git\"}},",
        "\"base\":{\"ref\":\"main\",\"repo\":{\"name\":\"base\",\"full_name\":\"alice/base\",\"private\":false,\"clone_url\":\"git@gitbucket.example.com:alice/base.git\"}}",
        "}"
    );
    let (port, server) = serve_json_once(
        "GET /api/v3/repos/alice/project/pulls/7 HTTP/1.1",
        "authorization: token test-token",
        body,
    );

    let output = env
        .repo_api_command(format!("127.0.0.1:{port}"), "alice/project")
        .current_dir(&local_repo)
        .args(["pr", "checkout", "7"])
        .output()
        .unwrap();

    server.join().unwrap();

    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        git_output(&local_repo, &["branch", "--show-current"]),
        "pr-7"
    );
    assert_eq!(
        git_output(&local_repo, &["rev-parse", "main"]),
        local_main_before
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Checked out branch 'pr-7'"),
        "stdout: {stdout}"
    );
}

#[test]
fn pr_diff_prefers_matching_remotes_when_api_clone_urls_are_unusable() {
    let env = GbTestEnv::new();
    let fixture = setup_pr_remote_fixture(
        env.path(),
        "feature/demo",
        "base\nfeature\n",
        "feature",
        false,
    );
    let local_repo = fixture.local_repo;

    let body = concat!(
        "{",
        "\"number\":5,",
        "\"title\":\"Feature\",",
        "\"state\":\"open\",",
        "\"head\":{\"ref\":\"feature/demo\",\"repo\":{\"name\":\"head\",\"full_name\":\"bob/head\",\"private\":true,\"clone_url\":\"git@gitbucket.example.com:bob/head.git\"}},",
        "\"base\":{\"ref\":\"main\",\"repo\":{\"name\":\"base\",\"full_name\":\"alice/base\",\"private\":false,\"clone_url\":\"git@gitbucket.example.com:alice/base.git\"}}",
        "}"
    );
    let (port, server) = serve_json_once(
        "GET /api/v3/repos/alice/project/pulls/5 HTTP/1.1",
        "authorization: token test-token",
        body,
    );

    let output = env
        .repo_api_command(format!("127.0.0.1:{port}"), "alice/project")
        .current_dir(&local_repo)
        .args(["pr", "diff", "5"])
        .output()
        .unwrap();

    server.join().unwrap();

    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("feature"), "stdout: {stdout}");
}
