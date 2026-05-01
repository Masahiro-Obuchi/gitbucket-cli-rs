use crate::support::gb_cmd::GbTestEnv;
use crate::support::git_repo::{
    clone_repo, commit_readme, init_bare_repo, init_work_repo, push_main,
};
use crate::support::mock_http::{spawn_scripted_server, ScriptedResponse};

#[test]
fn pr_diff_returns_non_zero_when_closed_pr_diff_is_unavailable() {
    let env = GbTestEnv::new();
    let remote = env.path().join("remote.git");
    init_bare_repo(&remote);

    let work = env.path().join("work");
    init_work_repo(&work);
    commit_readme(&work, "base\n", "base");
    push_main(&work, &remote);

    let local_repo = env.path().join("local-repo");
    clone_repo(env.path(), &remote, &local_repo);

    let (port, server) = spawn_scripted_server(vec![
        ScriptedResponse::json(
            "GET /api/v3/repos/alice/project/pulls/9 HTTP/1.1",
            "200 OK",
            "{\"number\":9,\"title\":\"Already merged\",\"state\":\"closed\",\"head\":{\"ref\":\"main\"},\"base\":{\"ref\":\"main\"}}",
        ),
        ScriptedResponse::json(
            "GET /api/v3/repos/alice/project/issues/9 HTTP/1.1",
            "404 Not Found",
            "{\"message\":\"not found\"}",
        ),
        ScriptedResponse {
            expected_request_line: "GET /alice/project/pull/9.diff HTTP/1.1".into(),
            status_line: "404 Not Found".into(),
            headers: vec![("content-type".into(), "text/plain".into())],
            body: "not found".into(),
        },
    ]);

    let output = env
        .repo_api_command(format!("127.0.0.1:{port}"), "alice/project")
        .current_dir(&local_repo)
        .args(["pr", "diff", "9", "--no-pager"])
        .output()
        .unwrap();

    let requests = server.join().unwrap();

    assert_eq!(requests.len(), 3);
    assert!(
        !output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        output.stdout.is_empty(),
        "stdout: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Diff unavailable"), "stderr: {stderr}");
    assert!(stderr.contains("pull request #9"), "stderr: {stderr}");
}

#[test]
fn pr_diff_uses_saved_diff_when_closed_branch_diff_is_empty() {
    let env = GbTestEnv::new();
    let remote = env.path().join("remote.git");
    init_bare_repo(&remote);

    let work = env.path().join("work");
    init_work_repo(&work);
    commit_readme(&work, "base\n", "base");
    push_main(&work, &remote);

    let local_repo = env.path().join("local-repo");
    clone_repo(env.path(), &remote, &local_repo);

    let (port, server) = spawn_scripted_server(vec![
        ScriptedResponse::json(
            "GET /api/v3/repos/alice/project/pulls/9 HTTP/1.1",
            "200 OK",
            "{\"number\":9,\"title\":\"Already merged\",\"state\":\"closed\",\"diff_url\":\"/alice/project/pull/9.diff\",\"head\":{\"ref\":\"main\"},\"base\":{\"ref\":\"main\"}}",
        ),
        ScriptedResponse {
            expected_request_line: "GET /alice/project/pull/9.diff HTTP/1.1".into(),
            status_line: "200 OK".into(),
            headers: vec![("content-type".into(), "text/plain".into())],
            body: "diff --git a/README.md b/README.md\n+saved\n".into(),
        },
    ]);

    let output = env
        .repo_api_command(format!("127.0.0.1:{port}"), "alice/project")
        .current_dir(&local_repo)
        .args(["pr", "diff", "9", "--no-pager"])
        .output()
        .unwrap();

    let requests = server.join().unwrap();

    assert_eq!(requests.len(), 2);
    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("+saved"), "stdout: {stdout}");
}

#[test]
fn pr_diff_rejects_non_diff_saved_diff_response() {
    let env = GbTestEnv::new();
    let remote = env.path().join("remote.git");
    init_bare_repo(&remote);

    let work = env.path().join("work");
    init_work_repo(&work);
    commit_readme(&work, "base\n", "base");
    push_main(&work, &remote);

    let local_repo = env.path().join("local-repo");
    clone_repo(env.path(), &remote, &local_repo);

    let (port, server) = spawn_scripted_server(vec![
        ScriptedResponse::json(
            "GET /api/v3/repos/alice/project/pulls/9 HTTP/1.1",
            "200 OK",
            "{\"number\":9,\"title\":\"Already merged\",\"state\":\"closed\",\"diff_url\":\"/alice/project/pull/9.diff\",\"head\":{\"ref\":\"main\"},\"base\":{\"ref\":\"main\"}}",
        ),
        ScriptedResponse {
            expected_request_line: "GET /alice/project/pull/9.diff HTTP/1.1".into(),
            status_line: "200 OK".into(),
            headers: vec![("content-type".into(), "text/html".into())],
            body: "<html><body>Please sign in</body></html>".into(),
        },
        ScriptedResponse {
            expected_request_line: "GET /alice/project/pull/9.diff HTTP/1.1".into(),
            status_line: "404 Not Found".into(),
            headers: vec![("content-type".into(), "text/plain".into())],
            body: "not found".into(),
        },
    ]);

    let output = env
        .repo_api_command(format!("127.0.0.1:{port}"), "alice/project")
        .current_dir(&local_repo)
        .args(["pr", "diff", "9", "--no-pager"])
        .output()
        .unwrap();

    let requests = server.join().unwrap();

    assert_eq!(requests.len(), 3);
    assert!(
        !output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        output.stdout.is_empty(),
        "stdout: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("response was not a diff"),
        "stderr: {stderr}"
    );
}
