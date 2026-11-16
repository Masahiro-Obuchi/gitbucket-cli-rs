mod support;
use support::gb_cmd::GbTestEnv;

#[test]
fn editor_sources_conflict_and_nonterminal_editor_never_starts() {
    for args in [
        vec!["issue", "create"],
        vec!["issue", "edit", "1"],
        vec!["issue", "comment", "1"],
        vec!["pr", "create"],
        vec!["pr", "edit", "1"],
        vec!["pr", "comment", "1"],
        vec![
            "pr", "comment", "edit", "1", "--kind", "inline", "--id", "2", "--web",
        ],
        vec![
            "pr", "comment", "reply", "1", "--kind", "inline", "--id", "2", "--web",
        ],
    ] {
        for other in [vec!["--body", ""], vec!["--body-file", "nonexistent"]] {
            let output = GbTestEnv::new()
                .command()
                .args(&args)
                .arg("--editor")
                .args(other)
                .output()
                .unwrap();
            assert_eq!(output.status.code(), Some(2));
        }
        let output = GbTestEnv::new()
            .command()
            .args(&args)
            .args(["--json-errors", "--editor"])
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(1));
        let error: serde_json::Value = serde_json::from_slice(&output.stderr).unwrap();
        assert_eq!(error["error"]["code"], "input_required");
    }
}

#[cfg(unix)]
#[test]
fn editor_fixture() {
    support::editor::fixture();
}

#[cfg(unix)]
mod terminal {
    use super::*;
    use serde_json::{json, Value};
    use support::{
        editor,
        gb_cmd::{assert_failure, assert_success},
        mock_http::{spawn_scripted_server, ScriptedResponse},
    };
    fn response(method: &str, path: &str, body: Value) -> ScriptedResponse {
        ScriptedResponse::json(
            &format!("{method} /api/v3/{path} HTTP/1.1"),
            "200 OK",
            &body.to_string(),
        )
    }

    #[test]
    fn editor_create_preserves_utf8_and_keeps_editor_output_out_of_json() {
        for kind in ["issue", "pr"] {
            let env = GbTestEnv::new();
            let text = "## 日本語\r\n\n  tail\n";
            let (port, server) = spawn_scripted_server(vec![response(
                "POST",
                &format!(
                    "repos/alice/project/{}",
                    if kind == "pr" { "pulls" } else { "issues" }
                ),
                json!({"number":1,"title":"new","state":"open","body":text}),
            )]);
            let mut command = env.repo_api_command(format!("127.0.0.1:{port}"), "alice/project");
            command.args([kind, "create", "--title", "new", "--editor"]);
            if kind == "pr" {
                command.args(["--head", "topic", "--base", "main", "--json"]);
            }
            let log = env.path().join("editor");
            editor::configure(&mut command, &log, text, "write");
            let output = editor::run(&mut command);
            assert_success(&output);
            assert!(!String::from_utf8_lossy(&output.stdout).contains("EDITOR-STDOUT"));
            if kind == "pr" {
                assert_eq!(
                    serde_json::from_slice::<Value>(&output.stdout).unwrap()["body"],
                    text
                );
            }
            let requests = server.join().unwrap();
            assert_eq!(
                serde_json::from_str::<Value>(&requests[0].body).unwrap()["body"],
                text
            );
            assert_eq!(std::fs::read(log.with_extension("seed")).unwrap(), b"");
            editor::assert_cleaned(&log);
        }
    }

    #[test]
    fn editor_edit_preloads_existing_descriptions_and_can_clear_them() {
        for kind in ["issue", "pr"] {
            for text in ["", "new\r\nbody\n"] {
                let env = GbTestEnv::new();
                let path = format!(
                    "repos/alice/project/{}/1",
                    if kind == "pr" { "pulls" } else { "issues" }
                );
                let before =
                    json!({"number":1,"title":"old","state":"open","body":"original\r\nbody\n"});
                let after = json!({"number":1,"title":"old","state":"open","body":text});
                let mut frames = vec![
                    response("GET", &path, before),
                    response("PATCH", &path, after.clone()),
                ];
                if kind == "issue" {
                    frames.push(response("GET", &path, after));
                }
                let (port, server) = spawn_scripted_server(frames);
                let mut command =
                    env.repo_api_command(format!("127.0.0.1:{port}"), "alice/project");
                command.args([kind, "edit", "1", "--editor"]);
                let log = env.path().join("editor");
                editor::configure(&mut command, &log, text, "write");
                assert_success(&editor::run(&mut command));
                assert_eq!(
                    std::fs::read_to_string(log.with_extension("seed")).unwrap(),
                    "original\r\nbody\n"
                );
                let requests = server.join().unwrap();
                assert_eq!(
                    serde_json::from_str::<Value>(&requests[1].body).unwrap()["body"],
                    text
                );
                editor::assert_cleaned(&log);
            }
        }
    }

    #[test]
    fn editor_comments_preload_only_the_authenticated_users_latest_comment() {
        for kind in ["issue", "pr"] {
            for edit in [false, true] {
                let env = GbTestEnv::new();
                let mut frames = Vec::new();
                if kind == "pr" {
                    frames.push(response(
                        "GET",
                        "repos/alice/project/pulls/1",
                        json!({"number":1,"title":"PR","state":"open"}),
                    ));
                }
                if edit {
                    frames.extend([
                response("GET", "user", json!({"login":"alice"})),
                response("GET", "repos/alice/project/issues/1/comments?per_page=100", json!([{"id":3,"body":"alice body","user":{"login":"alice"}},{"id":4,"body":"other","user":{"login":"bob"}}])),
            ]);
                }
                frames.push(response(
                    if edit { "PATCH" } else { "POST" },
                    if edit {
                        "repos/alice/project/issues/comments/3"
                    } else {
                        "repos/alice/project/issues/1/comments"
                    },
                    json!({"id":3,"body":"new comment\n"}),
                ));
                let (port, server) = spawn_scripted_server(frames);
                let mut command =
                    env.repo_api_command(format!("127.0.0.1:{port}"), "alice/project");
                command.args([kind, "comment", "1", "--editor"]);
                if edit {
                    command.arg("--edit-last");
                }
                let log = env.path().join("editor");
                editor::configure(&mut command, &log, "new comment\n", "write");
                assert_success(&editor::run(&mut command));
                assert_eq!(
                    std::fs::read_to_string(log.with_extension("seed")).unwrap(),
                    if edit { "alice body" } else { "" }
                );
                let requests = server.join().unwrap();
                assert_eq!(
                    serde_json::from_str::<Value>(&requests.last().unwrap().body).unwrap()["body"],
                    "new comment\n"
                );
                editor::assert_cleaned(&log);
            }
        }
    }

    #[test]
    fn editor_failures_unchanged_blank_and_signal_cancellation_never_contact_api() {
        for mode in ["fail", "unchanged", "invalid", "sleep", "blank"] {
            let env = GbTestEnv::new();
            let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            listener.set_nonblocking(true).unwrap();
            let mut command = env.repo_api_command(
                format!("127.0.0.1:{}", listener.local_addr().unwrap().port()),
                "alice/project",
            );
            command
                .env("GB_HTTP_TIMEOUT", "1")
                .args(["issue", "comment", "1", "--editor"]);
            let log = env.path().join("editor");
            editor::configure(
                &mut command,
                &log,
                " \n\t",
                if mode == "blank" { "write" } else { mode },
            );
            let output =
                editor::run_cancel(&mut command, (mode == "sleep").then_some(log.as_path()));
            assert_failure(&output);
            assert!(listener.accept().is_err(), "editor failure contacted API");
            editor::assert_cleaned(&log);
        }
    }

    #[test]
    fn json_errors_rejects_editor_even_with_terminals() {
        let env = GbTestEnv::new();
        let log = env.path().join("editor");
        let mut command = env.command();
        editor::configure(&mut command, &log, "body", "write");
        command.args([
            "issue",
            "create",
            "--title",
            "title",
            "--editor",
            "--json-errors",
        ]);
        let output = editor::run(&mut command);
        assert_failure(&output);
        assert!(!log.exists());
        let error: Value = serde_json::from_slice(&output.stderr).unwrap();
        assert_eq!(error["error"]["code"], "input_required");
    }

    #[test]
    fn editor_restores_terminal_modes_after_success_failure_and_cancellation() {
        for mode in ["raw_write", "raw_fail", "raw_unchanged", "raw_sleep"] {
            let env = GbTestEnv::new();
            let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            listener.set_nonblocking(true).unwrap();
            let (port, server) = if mode == "raw_write" {
                let (port, server) = spawn_scripted_server(vec![response(
                    "POST",
                    "repos/alice/project/issues/1/comments",
                    json!({"id":3,"body":"new body\n"}),
                )]);
                (port, Some(server))
            } else {
                (listener.local_addr().unwrap().port(), None)
            };
            let mut command = env.repo_api_command(format!("127.0.0.1:{port}"), "alice/project");
            command.args(["issue", "comment", "1", "--editor"]);
            let log = env.path().join("editor");
            editor::configure(&mut command, &log, "new body\n", mode);
            // run_cancel also verifies that the PTY's original modes survive.
            let output =
                editor::run_cancel(&mut command, (mode == "raw_sleep").then_some(log.as_path()));
            if let Some(server) = server {
                assert_success(&output);
                assert_eq!(server.join().unwrap().len(), 1);
            } else {
                assert_failure(&output);
                assert!(listener.accept().is_err());
            }
            editor::assert_cleaned(&log);
        }
    }
}
