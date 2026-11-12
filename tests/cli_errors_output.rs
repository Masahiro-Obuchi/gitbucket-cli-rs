mod support;

use serde_json::Value;
use std::process::Stdio;
use support::gb_cmd::{assert_failure, assert_success, GbTestEnv};

fn json_error(output: &std::process::Output, exit: i32) -> Value {
    assert_failure(output);
    assert_eq!(
        output.status.code(),
        Some(exit),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stdout.is_empty());
    serde_json::from_slice(&output.stderr).unwrap_or_else(|e| {
        panic!(
            "not one JSON error: {e}: {}",
            String::from_utf8_lossy(&output.stderr)
        )
    })
}

#[test]
fn argument_failures_use_json_and_exit_two_in_flag_and_environment_modes() {
    for (args, env_mode) in [
        (vec!["--json-errors", "pr", "view", "not-a-number"], false),
        (vec!["pr", "view", "not-a-number", "--json-errors"], false),
        (vec!["pr", "view", "7", "--unknown", "--json-errors"], false),
        (vec!["pr", "view"], true),
        (vec!["missing-command"], true),
    ] {
        let env = GbTestEnv::new();
        let mut cmd = env.command();
        cmd.env_remove("GB_JSON_ERRORS").args(args);
        if env_mode {
            cmd.env("GB_JSON_ERRORS", "true");
        }
        let output = cmd.output().unwrap();
        let error = json_error(&output, 2);
        assert_eq!(error["error"]["code"], "invalid_arguments");
        assert_eq!(error["error"]["exit_code"], 2);
    }
}

#[test]
fn flag_text_in_values_and_after_the_option_terminator_does_not_enable_json_errors() {
    for args in [
        vec!["pr", "comment", "7", "--body=--json-errors", "--unknown"],
        vec![
            "pr",
            "comment",
            "7",
            "--body",
            "text --json-errors text",
            "--unknown",
        ],
        vec!["pr", "comment", "7", "-b--json-errors", "--unknown"],
        vec!["pr", "view", "--", "--json-errors"],
    ] {
        let env = GbTestEnv::new();
        let out = env
            .command()
            .env("GB_JSON_ERRORS", "false")
            .args(args)
            .output()
            .unwrap();
        assert_eq!(out.status.code(), Some(2));
        assert!(serde_json::from_slice::<Value>(&out.stderr).is_err());
    }
}

#[test]
fn help_and_version_stay_normal_success_output_in_json_error_mode() {
    for args in [
        vec!["--json-errors", "--help"],
        vec!["pr", "--help"],
        vec!["--version"],
        vec!["help", "auth"],
    ] {
        let env = GbTestEnv::new();
        let out = env
            .command()
            .env("GB_JSON_ERRORS", "true")
            .args(args)
            .output()
            .unwrap();
        assert_success(&out);
        assert!(!out.stdout.is_empty());
        assert!(out.stderr.is_empty());
        assert!(serde_json::from_slice::<Value>(&out.stdout).is_err());
    }
}

#[test]
fn parsing_errors_never_echo_secret_argument_or_environment_values_in_json() {
    let env = GbTestEnv::new();
    let out = env
        .command()
        .env("GB_TOKEN", "secret-number-value")
        .args(["--json-errors", "pr", "view", "secret-number-value"])
        .output()
        .unwrap();
    let error = json_error(&out, 2);
    assert_eq!(error["error"]["code"], "invalid_arguments");
    assert!(!String::from_utf8_lossy(&out.stderr).contains("secret-number-value"));
}

#[test]
fn closed_stdout_during_completion_is_a_normal_pipeline_exit() {
    let env = GbTestEnv::new();
    let mut child = env
        .command()
        .env("GB_JSON_ERRORS", "true")
        .args(["completion", "bash"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    drop(child.stdout.take());
    let out = child.wait_with_output().unwrap();
    assert_success(&out);
    assert!(
        out.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[cfg(target_os = "linux")]
#[test]
fn non_pipe_stdout_errors_are_reported_without_panicking() {
    let env = GbTestEnv::new();
    let full = std::fs::OpenOptions::new()
        .write(true)
        .open("/dev/full")
        .unwrap();
    let out = env
        .command()
        .args(["--json-errors", "config", "path"])
        .stdout(full)
        .output()
        .unwrap();
    assert_failure(&out);
    let error: Value = serde_json::from_slice(&out.stderr).unwrap();
    assert_eq!(error["error"]["code"], "io_error");
    assert_eq!(error["error"]["exit_code"], 1);
}

#[cfg(target_os = "linux")]
#[test]
fn auth_status_preserves_probe_errors_when_stdout_also_fails() {
    use support::mock_http::{spawn_scripted_server, ScriptedResponse};
    for json in [false, true] {
        for rejected in [false, true] {
            let env = GbTestEnv::new();
            let (port, server) = spawn_scripted_server(vec![ScriptedResponse::json(
                "GET /api/v3/user HTTP/1.1",
                if rejected {
                    "401 Unauthorized"
                } else {
                    "200 OK"
                },
                if rejected {
                    r#"{"message":"rejected"}"#
                } else {
                    r#"{"login":"alice"}"#
                },
            )]);
            let full = std::fs::OpenOptions::new()
                .write(true)
                .open("/dev/full")
                .unwrap();
            let mut command = env.api_command(format!("127.0.0.1:{port}"));
            command
                .env_remove("GB_PROFILE")
                .args(["--json-errors", "auth", "status"])
                .stdout(full);
            if json {
                command.arg("--json");
            }
            let out = command.output().unwrap();
            assert_eq!(server.join().unwrap().len(), 1);
            let error = json_error(&out, 1);
            if rejected {
                assert_eq!(error["error"]["code"], "api_error", "{error}");
                assert_eq!(error["error"]["status"], 401);
            } else {
                assert_eq!(error["error"]["code"], "io_error");
                assert!(error["error"]["status"].is_null());
            }
        }
    }
}

#[cfg(target_os = "linux")]
#[test]
fn missing_authentication_is_not_hidden_by_status_json_write_failure() {
    let env = GbTestEnv::new();
    let full = std::fs::OpenOptions::new()
        .write(true)
        .open("/dev/full")
        .unwrap();
    let out = env
        .command()
        .env_remove("GB_HOST")
        .env_remove("GB_TOKEN")
        .env_remove("GB_PROFILE")
        .args(["--json-errors", "auth", "status", "--json"])
        .stdout(full)
        .output()
        .unwrap();
    let error = json_error(&out, 1);
    assert_eq!(error["error"]["code"], "not_authenticated");
}

#[test]
fn json_mode_accepts_boolish_environment_values_and_suppresses_noninteractive_prompts() {
    for value in ["true", "1", "yes", "on"] {
        let env = GbTestEnv::new();
        let out = env
            .command()
            .env("GB_JSON_ERRORS", value)
            .args(["auth", "login"])
            .stdin(Stdio::null())
            .output()
            .unwrap();
        let error = json_error(&out, 1);
        assert_eq!(error["error"]["code"], "input_required");
        assert_eq!(error["error"]["cause"], "interactive_input_disabled");
    }
}

#[test]
fn malformed_config_and_explicit_login_tokens_are_not_exposed_in_runtime_json_errors() {
    let env = GbTestEnv::new();
    std::fs::write(
        env.path().join("config.toml"),
        "token = 'private-unclosed-value",
    )
    .unwrap();
    let out = env
        .command()
        .env_remove("GB_JSON_ERRORS")
        .args([
            "--json-errors",
            "auth",
            "login",
            "-H",
            "example.invalid",
            "-t",
            "explicit-secret",
        ])
        .output()
        .unwrap();
    let error = json_error(&out, 1);
    assert_eq!(error["error"]["code"], "toml_deserialization_error");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(!stderr.contains("private-unclosed-value"));
    assert!(!stderr.contains("explicit-secret"));
}

#[test]
fn broken_stdout_is_handled_for_tables_and_json_after_network_reads() {
    use support::mock_http::{spawn_scripted_server, ScriptedResponse};
    for json in [false, true] {
        let env = GbTestEnv::new();
        let body = serde_json::json!([{"name":"project","full_name":"alice/project", "private":false, "description":"x".repeat(64*1024)}]).to_string();
        let (port, server) = spawn_scripted_server(vec![ScriptedResponse::json(
            "GET /api/v3/user/repos HTTP/1.1",
            "200 OK",
            &body,
        )]);
        let mut cmd = env.api_command(format!("127.0.0.1:{port}"));
        cmd.args(["--json-errors", "repo", "list"]);
        if json {
            cmd.arg("--json");
        }
        let mut child = cmd
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        drop(child.stdout.take());
        let out = child.wait_with_output().unwrap();
        server.join().unwrap();
        assert_success(&out);
        assert!(
            out.stderr.is_empty(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
}

#[cfg(unix)]
fn terminal() -> (std::fs::File, std::fs::File) {
    use std::os::fd::FromRawFd;
    unsafe {
        let (mut master, mut slave) = (-1, -1);
        assert_eq!(
            libc::openpty(
                &mut master,
                &mut slave,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            0
        );
        let flags = libc::fcntl(master, libc::F_GETFL);
        assert_ne!(
            libc::fcntl(master, libc::F_SETFL, flags | libc::O_NONBLOCK),
            -1
        );
        (
            std::fs::File::from_raw_fd(master),
            std::fs::File::from_raw_fd(slave),
        )
    }
}

#[cfg(unix)]
fn bounded_wait(child: &mut std::process::Child) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(8);
    while child.try_wait().unwrap().is_none() {
        if std::time::Instant::now() >= deadline {
            child.kill().unwrap();
            child.wait().unwrap();
            panic!("CLI did not close/reap its input or pager");
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}

#[cfg(unix)]
#[test]
fn json_errors_do_not_emit_prompt_control_sequences_even_on_a_terminal() {
    use std::io::Read;
    let env = GbTestEnv::new();
    let (mut master, slave) = terminal();
    let mut child = env
        .command()
        .args(["--json-errors", "auth", "login"])
        .stdin(slave.try_clone().unwrap())
        .stderr(slave.try_clone().unwrap())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    bounded_wait(&mut child);
    let out = child.wait_with_output().unwrap();
    assert!(out.stdout.is_empty());
    assert_eq!(out.status.code(), Some(1));
    let mut stderr = Vec::new();
    let mut bytes = [0; 4096];
    while let Ok(count) = master.read(&mut bytes) {
        if count == 0 {
            break;
        }
        stderr.extend_from_slice(&bytes[..count]);
    }
    // Retain a parent slave until draining is complete: closing the last slave
    // at child exit can discard unread PTY output on the macOS CI runner.
    drop(slave);
    let error: Value = serde_json::from_slice(&stderr).unwrap();
    assert_eq!(error["error"]["code"], "input_required");
}

#[cfg(unix)]
#[test]
fn pager_success_early_exit_failure_and_missing_program_preserve_error_stream_contracts() {
    use support::mock_http::{spawn_scripted_server, ScriptedResponse};
    for mode in ["consume", "early", "failure", "missing"] {
        let env = GbTestEnv::new();
        let pid = env.path().join("pager.pid");
        let script = env.path().join("pager.sh");
        std::fs::write(&script, "#!/bin/sh\necho $$ > \"$GB_TEST_PAGER_PID\"\nprintf 'raw pager diagnostic\\n' >&2\ncase \"$GB_TEST_PAGER_MODE\" in\n consume) cat >/dev/null ;;\n failure) exit 7 ;;\nesac\n").unwrap();
        let (_master, slave) = terminal();
        let (port, server) = spawn_scripted_server(vec![ScriptedResponse::json(
            "GET /api/v3/repos/alice/project/issues?state=open HTTP/1.1",
            "200 OK",
            "[]",
        )]);
        let mut child = env
            .repo_api_command(format!("127.0.0.1:{port}"), "alice/project")
            .args(["--json-errors", "issue", "list"])
            .env(
                "PAGER",
                if mode == "missing" {
                    env.path().join("absent-pager").display().to_string()
                } else {
                    format!("/bin/sh {}", script.display())
                },
            )
            .env("GB_TEST_PAGER_PID", &pid)
            .env("GB_TEST_PAGER_MODE", mode)
            .stdout(slave)
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        bounded_wait(&mut child);
        let out = child.wait_with_output().unwrap();
        server.join().unwrap();
        if mode == "failure" {
            let error = json_error(&out, 1);
            assert_eq!(error["error"]["code"], "pager_failed");
            assert_eq!(error["error"]["child_exit_code"], 7);
        } else {
            assert_success(&out);
            assert!(
                out.stderr.is_empty(),
                "{}",
                String::from_utf8_lossy(&out.stderr)
            );
        }
        if mode != "missing" {
            let pid: i32 = std::fs::read_to_string(pid)
                .unwrap()
                .trim()
                .parse()
                .unwrap();
            assert_eq!(unsafe { libc::kill(pid, 0) }, -1, "pager was not reaped");
        }
    }
}

#[test]
fn child_process_failure_is_one_json_error_without_raw_git_stderr() {
    let env = GbTestEnv::new();
    let missing = env.path().join("missing-secret-child.git");
    let url = url::Url::from_directory_path(missing).unwrap().to_string();
    let out = env
        .command()
        .env("GB_TOKEN", "secret-child")
        .args(["--json-errors", "repo", "clone", &url])
        .output()
        .unwrap();
    let error = json_error(&out, 1);
    assert_eq!(error["error"]["exit_code"], 1);
    assert!(!String::from_utf8_lossy(&out.stderr).contains("secret-child"));
}

#[test]
fn runtime_failures_do_not_become_success_merely_because_stdout_is_closed() {
    let env = GbTestEnv::new();
    let mut child = env
        .command()
        .args(["--json-errors", "config", "get", "default-host"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    drop(child.stdout.take());
    let out = child.wait_with_output().unwrap();
    let error = json_error(&out, 1);
    assert_eq!(error["error"]["code"], "config_error");
}
