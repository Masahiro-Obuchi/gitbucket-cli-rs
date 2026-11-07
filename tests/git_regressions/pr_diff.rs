use crate::support::gb_cmd::{assert_failure, assert_success, GbTestEnv};
use crate::support::git_repo::{
    clone_repo, commit_readme, git_output, init_bare_repo, init_work_repo, push_main,
};
use crate::support::mock_http::{spawn_scripted_server, ScriptedResponse};
use serde_json::json;

#[test]
fn pr_diff_requires_saved_shas_without_probing_a_guessed_diff_route() {
    let env = GbTestEnv::new();
    let (port, server) = spawn_scripted_server(vec![ScriptedResponse::json(
        "GET /api/v3/repos/alice/project/pulls/9 HTTP/1.1",
        "200 OK",
        r#"{"number":9,"title":"Closed PR","state":"closed","head":{"ref":"main"},"base":{"ref":"main"}}"#,
    )]);
    let output = env
        .repo_api_command(format!("127.0.0.1:{port}"), "alice/project")
        .args(["--json-errors", "pr", "diff", "9", "--no-pager"])
        .output()
        .unwrap();
    assert_failure(&output);
    assert!(output.stdout.is_empty());
    let error: serde_json::Value = serde_json::from_slice(&output.stderr).unwrap();
    assert_eq!(error["error"]["code"], "diff_unavailable");
    assert_eq!(error["error"]["cause"], "missing_snapshot");
    assert_eq!(server.join().unwrap().len(), 1);
}

#[test]
fn pr_diff_keeps_a_verified_empty_snapshot_instead_of_an_advertised_text_diff() {
    let env = GbTestEnv::new();
    let remote = env.path().join("remote.git");
    let work = env.path().join("work");
    let local = env.path().join("local");
    init_bare_repo(&remote);
    init_work_repo(&work);
    commit_readme(&work, "base\n", "base");
    push_main(&work, &remote);
    clone_repo(env.path(), &remote, &local);
    let sha = git_output(&work, &["rev-parse", "HEAD"]);
    let payload = json!({"number":9,"title":"Merged","state":"closed","merged":true,
        "diff_url":"/unrelated.diff", "head":{"ref":"main","sha":sha},"base":{"ref":"main","sha":sha}});
    let (port, server) = spawn_scripted_server(vec![ScriptedResponse::json(
        "GET /api/v3/repos/alice/project/pulls/9 HTTP/1.1",
        "200 OK",
        &payload.to_string(),
    )]);
    let output = env
        .repo_api_command(format!("127.0.0.1:{port}"), "alice/project")
        .current_dir(&local)
        .args(["pr", "diff", "9", "--no-pager"])
        .output()
        .unwrap();
    assert_success(&output);
    assert!(output.stdout.is_empty());
    assert_eq!(server.join().unwrap().len(), 1);
}
