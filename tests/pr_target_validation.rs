mod support;

use serde_json::json;
use support::gb_cmd::{assert_failure, stderr, GbTestEnv};
use support::mock_http::{spawn_scripted_server, ScriptedResponse};

#[test]
fn numbered_pr_operations_validate_the_target_before_using_issue_subresources() {
    let actions = [
        vec![
            "pr",
            "edit",
            "5",
            "--title",
            "wrong",
            "--add-assignee",
            "alice",
            "--web",
        ],
        vec!["pr", "close", "5"],
        vec!["pr", "reopen", "5"],
        vec!["pr", "merge", "5"],
        vec!["pr", "comment", "list", "5", "--json"],
        vec!["pr", "comment", "5"],
        vec!["pr", "comment", "5", "--body", "must not post"],
        vec![
            "pr",
            "comment",
            "5",
            "--edit-last",
            "--body",
            "must not edit",
        ],
        vec!["pr", "view", "5", "--comments"],
        vec!["pr", "view", "5", "--web"],
        vec!["pr", "checkout", "5"],
        vec!["pr", "diff", "5"],
    ];
    for mode in ["404", "null", "mismatch", "forbidden"] {
        for args in &actions {
            let env = GbTestEnv::new();
            let (status, body) = match mode {
                "404" => ("404 Not Found", json!({"message":"not a PR"})),
                // The pinned server returns 200/null for an ordinary Issue number.
                "null" => ("200 OK", json!(null)),
                "mismatch" => (
                    "200 OK",
                    json!({"number":6,"title":"another PR","state":"open"}),
                ),
                _ => ("403 Forbidden", json!({"message":"denied"})),
            };
            let mut responses = vec![ScriptedResponse::json(
                "GET /bucket/api/v3/repos/alice/project/pulls/5 HTTP/1.1",
                status,
                &body.to_string(),
            )];
            if mode == "null" {
                responses.push(ScriptedResponse::json(
                    "GET /bucket/api/v3/repos/alice/project/pulls?state=all HTTP/1.1",
                    "200 OK",
                    "[]",
                ));
            }
            let (port, server) = spawn_scripted_server(responses);
            let output = env
                .repo_api_command(format!("127.0.0.1:{port}/bucket"), "alice/project")
                .env("GB_USER", "alice")
                .env("GB_PASSWORD", "must-not-be-used")
                .stdin(std::process::Stdio::null())
                .args(args)
                .output()
                .unwrap();
            assert_failure(&output);
            assert!(
                output.stdout.is_empty(),
                "{args:?}: {}",
                String::from_utf8_lossy(&output.stdout)
            );
            let message = stderr(&output);
            assert!(
                message.contains(match mode {
                    "404" => "API error (404)",
                    "null" => "Pull request #5 not found",
                    "mismatch" => "PR target mismatch",
                    _ => "API error (403)",
                }),
                "{args:?}, {mode}: {message}"
            );
            let requests = server.join().unwrap();
            assert_eq!(requests.len(), if mode == "null" { 2 } else { 1 });
            assert!(requests
                .iter()
                .all(|request| request.method == "GET" && request.target.contains("/pulls")));
        }
    }
}
