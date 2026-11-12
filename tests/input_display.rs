mod support;
use serde_json::{json, Value};
use std::{
    io::Write,
    process::{Command, Output, Stdio},
};
use support::{
    gb_cmd::{assert_failure, assert_success, GbTestEnv},
    git_repo::*,
    mock_http::{spawn_scripted_server, ScriptedResponse},
};

fn response(method: &str, path: &str, body: Value) -> ScriptedResponse {
    ScriptedResponse::json(
        &format!("{method} /api/v3/{path} HTTP/1.1"),
        "200 OK",
        &body.to_string(),
    )
}
fn stdin(command: &mut Command, text: &[u8]) -> Output {
    let mut child = command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(text).unwrap();
    child.wait_with_output().unwrap()
}

#[test]
fn explicit_body_sources_conflict_before_any_io() {
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
    ] {
        let output = GbTestEnv::new()
            .command()
            .args(args)
            .args(["--body", "a", "--body-file", "nonexistent"])
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(2));
    }
}

#[test]
fn create_preserves_file_and_stdin_markdown_and_optional_body_omission() {
    for kind in ["issue", "pr"] {
        for mode in ["file", "stdin", "omitted", "empty"] {
            let env = GbTestEnv::new();
            let text = "# 日本語 👩‍💻\r\n\n  indented\n\tcode\n\x1b[2J\n";
            let file = env.path().join("body.md");
            std::fs::write(&file, if mode == "empty" { "" } else { text }).unwrap();
            let path = if kind == "pr" { "pulls" } else { "issues" };
            let (port, server) = spawn_scripted_server(vec![response(
                "POST",
                &format!("repos/alice/project/{path}"),
                json!({"number":1,"title":"title","state":"open"}),
            )]);
            let mut command = env.repo_api_command(format!("127.0.0.1:{port}"), "alice/project");
            command.args([kind, "create", "--title", "title"]);
            if kind == "pr" {
                command.args(["--head", "topic", "--base", "main"]);
            }
            if mode == "file" || mode == "empty" {
                command.arg("--body-file").arg(&file);
            }
            if mode == "stdin" {
                command.args(["--body-file", "-"]);
            }
            let output = if mode == "stdin" {
                stdin(&mut command, text.as_bytes())
            } else {
                command.output().unwrap()
            };
            assert_success(&output);
            let requests = server.join().unwrap();
            let body: Value = serde_json::from_str(&requests[0].body).unwrap();
            match mode {
                "omitted" => assert!(body.get("body").is_none()),
                "empty" => assert_eq!(body["body"], ""),
                _ => assert_eq!(body["body"], text),
            }
        }
    }
}

#[test]
fn edits_clear_body_from_an_empty_file_and_preserve_multiline_stdin() {
    for kind in ["issue", "pr"] {
        for text in ["", "\n## heading\r\n\ntext  \n"] {
            let env = GbTestEnv::new();
            let file = env.path().join("empty.md");
            std::fs::write(&file, "").unwrap();
            let path = format!(
                "repos/alice/project/{}/1",
                if kind == "pr" { "pulls" } else { "issues" }
            );
            let before = json!({"number":1,"title":"old","state":"open","body":"before"});
            let after = json!({"number":1,"title":"old","state":"open","body":text});
            let mut responses = vec![
                response("GET", &path, before),
                response("PATCH", &path, after.clone()),
            ];
            if kind == "issue" {
                responses.push(response("GET", &path, after));
            }
            let (port, server) = spawn_scripted_server(responses);
            let mut command = env.repo_api_command(format!("127.0.0.1:{port}"), "alice/project");
            command.args([kind, "edit", "1", "--body-file"]);
            let output = if text.is_empty() {
                command.arg(file).output().unwrap()
            } else {
                stdin(command.arg("-"), text.as_bytes())
            };
            assert_success(&output);
            let requests = server.join().unwrap();
            let body: Value = serde_json::from_str(&requests[1].body).unwrap();
            assert_eq!(body["body"], text);
        }
    }
}

#[test]
fn discussion_comments_preserve_multiline_stdin() {
    for kind in ["issue", "pr"] {
        let env = GbTestEnv::new();
        let text = "\n日本語\r\n\n```rust\nlet x = 1;\n```\n";
        let mut responses = Vec::new();
        if kind == "pr" {
            responses.push(response(
                "GET",
                "repos/alice/project/pulls/1",
                json!({"number":1,"title":"title","state":"open"}),
            ));
        }
        responses.push(response(
            "POST",
            "repos/alice/project/issues/1/comments",
            json!({"id":2,"body":text}),
        ));
        let (port, server) = spawn_scripted_server(responses);
        let output = stdin(
            env.repo_api_command(format!("127.0.0.1:{port}"), "alice/project")
                .args([kind, "comment", "1", "--body-file", "-"]),
            text.as_bytes(),
        );
        assert_success(&output);
        let requests = server.join().unwrap();
        assert_eq!(
            serde_json::from_str::<Value>(&requests.last().unwrap().body).unwrap()["body"],
            text
        );
    }
}

#[test]
fn noninteractive_missing_input_blank_comments_and_bad_utf8_fail_before_mutation() {
    for args in [
        vec!["issue", "create"],
        vec!["pr", "create", "--head", "topic", "--base", "main"],
        vec!["issue", "comment", "1"],
        vec!["pr", "comment", "1"],
    ] {
        let env = GbTestEnv::new();
        let output = env
            .repo_api_command("127.0.0.1:1", "alice/project")
            .arg("--json-errors")
            .args(args)
            .output()
            .unwrap();
        assert_failure(&output);
        let error: Value = serde_json::from_slice(&output.stderr).unwrap();
        assert_eq!(error["error"]["code"], "input_required");
    }
    for kind in ["issue", "pr"] {
        let env = GbTestEnv::new();
        let output = stdin(
            env.repo_api_command("127.0.0.1:1", "alice/project").args([
                kind,
                "comment",
                "1",
                "--body-file",
                "-",
            ]),
            b" \n\t",
        );
        assert_failure(&output);
        assert!(String::from_utf8_lossy(&output.stderr).contains("must not be blank"));
        let output = stdin(
            env.repo_api_command("127.0.0.1:1", "alice/project").args([
                kind,
                "create",
                "--title",
                "x",
                "--body-file",
                "-",
            ]),
            &[0xff],
        );
        assert_failure(&output);
        assert!(!String::from_utf8_lossy(&output.stderr).contains("HTTP"));
    }
}

#[test]
fn token_stdin_verifies_and_stores_without_echoing_the_secret() {
    let env = GbTestEnv::new();
    let (port, server) =
        spawn_scripted_server(vec![response("GET", "user", json!({"login":"alice"}))]);
    let output = stdin(
        env.command().args([
            "auth",
            "login",
            "--hostname",
            &format!("127.0.0.1:{port}"),
            "--protocol",
            "http",
            "--with-token",
        ]),
        b"stdin-secret\r\n",
    );
    assert_success(&output);
    assert!(!String::from_utf8_lossy(&output.stdout).contains("stdin-secret"));
    assert!(!String::from_utf8_lossy(&output.stderr).contains("stdin-secret"));
    assert_eq!(
        server.join().unwrap()[0].headers["authorization"],
        "token stdin-secret"
    );
    assert!(std::fs::read_to_string(env.path().join("config.toml"))
        .unwrap()
        .contains("stdin-secret"));
    let (port, server) = spawn_scripted_server(vec![ScriptedResponse::json(
        "GET /api/v3/user HTTP/1.1",
        "401 Unauthorized",
        r#"{"message":"rejected stdin-secret"}"#,
    )]);
    let output = stdin(
        env.command().args([
            "--json-errors",
            "auth",
            "login",
            "--hostname",
            &format!("127.0.0.1:{port}"),
            "--protocol",
            "http",
            "--with-token",
        ]),
        b"stdin-secret",
    );
    assert_failure(&output);
    let error: Value = serde_json::from_slice(&output.stderr).unwrap();
    assert!(!error.to_string().contains("stdin-secret"));
    server.join().unwrap();
    for input in [
        b"".as_slice(),
        b"secret\nsecond",
        b"secret\0",
        b"secret\t",
        &[0xff],
    ] {
        let output = stdin(
            env.command().args([
                "--json-errors",
                "auth",
                "login",
                "--hostname",
                "127.0.0.1:1",
                "--with-token",
            ]),
            input,
        );
        assert_failure(&output);
        let error: Value = serde_json::from_slice(&output.stderr).unwrap();
        assert!(!error.to_string().contains("secret"));
    }
    let output = env
        .command()
        .args(["auth", "login", "--token", "old", "--with-token"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
}

#[test]
fn default_base_uses_server_metadata_and_never_guesses_main() {
    for base in [Some("master"), Some("develop"), None] {
        let env = GbTestEnv::new();
        let mut responses = vec![response(
            "GET",
            "repos/alice/project",
            json!({"name":"project","full_name":"alice/project","default_branch":base}),
        )];
        if base.is_some() {
            responses.push(response(
                "POST",
                "repos/alice/project/pulls",
                json!({"number":1,"title":"title","state":"open"}),
            ));
        }
        let (port, server) = spawn_scripted_server(responses);
        let output = env
            .repo_api_command(format!("127.0.0.1:{port}"), "alice/project")
            .args(["pr", "create", "--head", "topic", "--title", "title"])
            .output()
            .unwrap();
        let requests = server.join().unwrap();
        if let Some(base) = base {
            assert_success(&output);
            assert_eq!(
                serde_json::from_str::<Value>(&requests[1].body).unwrap()["base"],
                base
            );
        } else {
            assert_failure(&output);
            assert!(String::from_utf8_lossy(&output.stderr).contains("--base"));
        }
    }
}

#[test]
fn inferred_head_checks_live_ref_and_never_pushes_or_updates_tracking_refs() {
    for mode in [
        "published",
        "dirty",
        "ahead",
        "behind",
        "diverged",
        "missing",
    ] {
        let env = GbTestEnv::new();
        let local = env.path().join("local");
        let remote = env.path().join("remote.git");
        init_bare_repo(&remote);
        init_work_repo(&local);
        commit_readme(&local, "one\n", "one");
        run_git(&local, &["branch", "-M", "topic"]);
        run_git(
            &local,
            &["remote", "add", "origin", remote.to_str().unwrap()],
        );
        if mode != "missing" {
            run_git(&local, &["push", "-u", "origin", "topic"]);
        }
        if mode == "dirty" {
            std::fs::write(local.join("README.md"), "dirty\n").unwrap();
        }
        if matches!(mode, "ahead" | "behind" | "diverged") {
            let original = git_output(&local, &["rev-parse", "HEAD"]);
            commit_tracked_readme(&local, "two\n", "two");
            if mode != "ahead" {
                run_git(&local, &["push", "origin", "topic"]);
                // Preserve the local files, while moving only this disposable fixture's branch.
                run_git(&local, &["checkout", "-B", "topic", &original]);
                if mode == "diverged" {
                    commit_tracked_readme(&local, "three\n", "three");
                }
            }
        }
        let refs_before = git_output(&local, &["show-ref"]);
        let index_before = std::fs::read(local.join(".git/index")).unwrap();
        let published_before = git_output(&local, &["ls-remote", "origin"]);
        let succeeds = matches!(mode, "published" | "dirty");
        let mut responses = vec![response(
            "GET",
            "repos/alice/project",
            json!({"name":"project","full_name":"alice/project","default_branch":"develop","clone_url":remote.to_str().unwrap()}),
        )];
        if succeeds {
            responses.push(response(
                "POST",
                "repos/alice/project/pulls",
                json!({"number":1,"title":"title","state":"open"}),
            ));
        }
        let (port, server) = spawn_scripted_server(responses);
        let output = env
            .repo_api_command(format!("127.0.0.1:{port}"), "alice/project")
            .current_dir(&local)
            .args(["pr", "create", "--title", "title"])
            .output()
            .unwrap();
        if succeeds {
            assert_success(&output);
        } else {
            assert_failure(&output);
            assert!(String::from_utf8_lossy(&output.stderr).contains("No PR was created"));
        }
        if mode == "dirty" {
            assert!(String::from_utf8_lossy(&output.stderr).contains("Uncommitted changes"));
        }
        server.join().unwrap();
        assert_eq!(git_output(&local, &["show-ref"]), refs_before);
        assert_eq!(
            std::fs::read(local.join(".git/index")).unwrap(),
            index_before
        );
        assert_eq!(
            git_output(&local, &["ls-remote", "origin"]),
            published_before
        );
    }
}

#[test]
fn human_tables_views_and_success_lines_escape_controls_while_json_preserves_originals() {
    let title = "日本e\u{301}👩‍💻\n\x1b[2J\rspoof";
    let body = "first\n\tcode\r\x1b]8;;https://example.test\x07\u{202e}last\n";
    let item = json!({"number":1,"title":title,"body":body,"state":"open"});
    for (args, path, payload, is_json, list) in [
        (
            vec!["issue", "list"],
            "issues?state=open",
            json!([item]),
            false,
            true,
        ),
        (
            vec!["issue", "view", "1"],
            "issues/1",
            item.clone(),
            false,
            false,
        ),
        (
            vec!["issue", "view", "1", "--json"],
            "issues/1",
            item.clone(),
            true,
            false,
        ),
        (
            vec!["pr", "view", "1"],
            "pulls/1",
            item.clone(),
            false,
            false,
        ),
        (
            vec!["pr", "view", "1", "--json"],
            "pulls/1",
            item.clone(),
            true,
            false,
        ),
    ] {
        let env = GbTestEnv::new();
        let (port, server) = spawn_scripted_server(vec![response(
            "GET",
            &format!("repos/alice/project/{path}"),
            payload,
        )]);
        let output = env
            .repo_api_command(format!("127.0.0.1:{port}"), "alice/project")
            .args(args)
            .output()
            .unwrap();
        assert_success(&output);
        server.join().unwrap();
        if is_json {
            assert_eq!(
                serde_json::from_slice::<Value>(&output.stdout).unwrap(),
                item
            );
        } else {
            let out = String::from_utf8_lossy(&output.stdout);
            assert!(!out.contains(['\x1b', '\r', '\u{202e}']));
            if list {
                assert_eq!(out.lines().count(), 2);
            } else {
                assert!(out.lines().next().unwrap().contains("\\n"));
                assert!(out.contains("first\n\tcode\\r"));
            }
        }
    }
    let env = GbTestEnv::new();
    let (port, server) =
        spawn_scripted_server(vec![response("POST", "repos/alice/project/issues", item)]);
    let output = env
        .repo_api_command(format!("127.0.0.1:{port}"), "alice/project")
        .args(["issue", "create", "--title", "title"])
        .output()
        .unwrap();
    assert_success(&output);
    let out = String::from_utf8_lossy(&output.stdout);
    assert_eq!(out.lines().count(), 1);
    assert!(!out.contains(['\x1b', '\r']));
    server.join().unwrap();
}
