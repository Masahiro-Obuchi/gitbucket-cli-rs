mod support;

use serde_json::{json, Value};
use std::fs;
use std::path::PathBuf;
use std::process::Output;
use support::gb_cmd::{assert_failure, assert_success, GbTestEnv};
use support::git_repo::{
    commit_readme, commit_tracked_readme, git_output, init_bare_repo, init_work_repo, push_main,
    run_git,
};
use support::mock_http::{spawn_scripted_server, ScriptedResponse};

struct Fixture {
    env: GbTestEnv,
    source: PathBuf,
    remote: PathBuf,
    local: PathBuf,
    base: String,
    head: String,
}

impl Fixture {
    fn new(shallow: bool) -> Self {
        let env = GbTestEnv::new();
        let remote = env.path().join("remote.git");
        let source = env.path().join("source");
        let local = env.path().join("local");
        init_bare_repo(&remote);
        init_work_repo(&source);
        commit_readme(&source, "base\n", "base");
        push_main(&source, &remote);
        let base = git_output(&source, &["rev-parse", "HEAD"]);
        run_git(&source, &["checkout", "-b", "feature"]);
        commit_tracked_readme(&source, "base\noriginal PR change\n", "PR head");
        let head = git_output(&source, &["rev-parse", "HEAD"]);
        run_git(&source, &["push", "origin", "feature"]);
        run_git(&remote, &["update-ref", "refs/pull/7/head", &head]);
        let url = url::Url::from_directory_path(&remote).unwrap();
        let mut args = vec!["clone", "--single-branch", "--branch", "main"];
        if shallow {
            args.extend(["--depth", "1"]);
        }
        args.extend([url.as_str(), local.to_str().unwrap()]);
        run_git(env.path(), &args);
        Self {
            env,
            source,
            remote,
            local,
            base,
            head,
        }
    }

    fn payload(&self, state: &str) -> Value {
        let repository = json!({"name":"project","full_name":"alice/project","clone_url":url::Url::from_directory_path(&self.remote).unwrap().as_str()});
        json!({"number":7,"title":"snapshot","state":state,"merged":state=="closed",
            "head":{"ref":"feature","sha":self.head,"repo":repository},
            "base":{"ref":"main","sha":self.base,"repo":repository}})
    }

    fn run(&self, payload: Value, command: &str) -> Output {
        let (port, server) = spawn_scripted_server(vec![ScriptedResponse::json(
            "GET /api/v3/repos/alice/project/pulls/7 HTTP/1.1",
            "200 OK",
            &payload.to_string(),
        )]);
        let mut cmd = self
            .env
            .repo_api_command(format!("127.0.0.1:{port}"), "alice/project");
        cmd.current_dir(&self.local)
            .args(["--json-errors", "pr", command, "7"]);
        if command == "diff" {
            cmd.arg("--no-pager");
        }
        let output = cmd.output().unwrap();
        server.join().unwrap();
        output
    }
}

#[test]
fn moving_branches_never_replace_the_saved_open_closed_or_merged_comparison() {
    for (state, merged) in [("open", false), ("closed", false), ("closed", true)] {
        let f = Fixture::new(false);
        commit_tracked_readme(
            &f.source,
            "base\noriginal PR change\nlater head change\n",
            "reuse head",
        );
        run_git(&f.source, &["push", "origin", "feature"]);
        run_git(&f.source, &["checkout", "main"]);
        commit_tracked_readme(&f.source, "new base unrelated to PR\n", "advance base");
        run_git(&f.source, &["push", "origin", "main"]);
        let mut payload = f.payload(state);
        payload["merged"] = json!(merged);
        let output = f.run(payload, "diff");
        assert_success(&output);
        let diff = String::from_utf8_lossy(&output.stdout);
        assert!(diff.contains("+original PR change"), "{diff}");
        assert!(
            !diff.contains("later head change") && !diff.contains("new base unrelated"),
            "{diff}"
        );
        assert_eq!(git_output(&f.local, &["branch", "--show-current"]), "main");
    }
}

#[test]
fn retained_pr_ref_recovers_a_deleted_or_force_pushed_head_in_a_shallow_clone() {
    for delete in [false, true] {
        let f = Fixture::new(true);
        run_git(&f.source, &["reset", "--hard", "main"]);
        commit_tracked_readme(&f.source, "force pushed unrelated change\n", "replace head");
        run_git(&f.source, &["push", "--force", "origin", "feature"]);
        if delete {
            run_git(&f.source, &["push", "origin", "--delete", "feature"]);
        }
        let output = f.run(f.payload("closed"), "checkout");
        assert_success(&output);
        assert!(String::from_utf8_lossy(&output.stdout).contains(&f.head));
        assert_eq!(git_output(&f.local, &["rev-parse", "HEAD"]), f.head);
        assert_eq!(
            fs::read_to_string(f.local.join("README.md")).unwrap(),
            "base\noriginal PR change\n"
        );
    }
}

#[test]
fn null_or_verified_empty_detail_recovers_saved_shas_without_trusting_fork_metadata() {
    for body in ["null", ""] {
        let f = Fixture::new(false);
        let mut listed = f.payload("closed");
        listed["head"]["repo"]["clone_url"] = json!("https://unrelated.invalid/wrong/repo.git");
        let (port, server) = spawn_scripted_server(vec![
            ScriptedResponse::json(
                "GET /api/v3/repos/alice/project/pulls/7 HTTP/1.1",
                "200 OK",
                body,
            ),
            ScriptedResponse::json(
                "GET /api/v3/repos/alice/project/pulls?state=all HTTP/1.1",
                "200 OK",
                &json!([listed]).to_string(),
            ),
        ]);
        let output = f
            .env
            .repo_api_command(format!("127.0.0.1:{port}"), "alice/project")
            .current_dir(&f.local)
            .args(["pr", "diff", "7", "--no-pager"])
            .output()
            .unwrap();
        assert_success(&output);
        assert!(String::from_utf8_lossy(&output.stdout).contains("+original PR change"));
        assert_eq!(server.join().unwrap().len(), 2);
    }
}

#[test]
fn unavailable_or_invalid_snapshots_never_print_a_current_tip_or_saved_text_diff() {
    let f = Fixture::new(false);
    for invalid in [Value::Null, json!("HEAD"), json!("1".repeat(40))] {
        let mut payload = f.payload("closed");
        payload["head"]["sha"] = invalid;
        payload["diff_url"] = json!("/wrong.diff?token=must-not-leak");
        let output = f.run(payload, "diff");
        assert_failure(&output);
        assert!(output.stdout.is_empty());
        let error: Value = serde_json::from_slice(&output.stderr).unwrap();
        assert_eq!(error["error"]["code"], "diff_unavailable");
        assert!(!String::from_utf8_lossy(&output.stderr).contains("must-not-leak"));
        assert_eq!(git_output(&f.local, &["branch", "--show-current"]), "main");
    }
}

#[test]
fn replacement_refs_cannot_change_snapshot_contents() {
    let f = Fixture::new(false);
    // First load the saved objects, then install a local replacement of their
    // content. The second invocation must still use the literal server SHA.
    assert_success(&f.run(f.payload("closed"), "diff"));
    support::git_repo::configure_test_user(&f.local);
    commit_tracked_readme(&f.local, "replacement contents\n", "replacement");
    let replacement = git_output(&f.local, &["rev-parse", "HEAD"]);
    run_git(&f.local, &["replace", &f.head, &replacement]);
    let output = f.run(f.payload("closed"), "diff");
    assert_success(&output);
    let diff = String::from_utf8_lossy(&output.stdout);
    assert!(
        diff.contains("+original PR change") && !diff.contains("replacement contents"),
        "{diff}"
    );
}

#[test]
fn equal_saved_trees_are_a_valid_empty_diff_even_for_closed_prs() {
    let f = Fixture::new(false);
    let mut payload = f.payload("closed");
    payload["head"]["sha"] = json!(f.base);
    let output = f.run(payload, "diff");
    assert_success(&output);
    assert!(output.stdout.is_empty());
}

#[test]
fn binary_rename_and_large_diffs_are_complete_even_from_a_subdirectory() {
    let mut f = Fixture::new(false);
    fs::write(f.source.join("README.md"), "base\n").unwrap();
    run_git(&f.source, &["mv", "README.md", "renamed file.txt"]);
    let mut state = 0x12345678_u32;
    let bytes: Vec<u8> = (0..2_000_000)
        .map(|_| {
            state ^= state << 13;
            state ^= state >> 17;
            state ^= state << 5;
            state as u8
        })
        .collect();
    fs::write(f.source.join("binary.dat"), bytes).unwrap();
    run_git(&f.source, &["add", "."]);
    run_git(&f.source, &["commit", "-m", "rename and binary data"]);
    f.head = git_output(&f.source, &["rev-parse", "HEAD"]);
    run_git(&f.source, &["push", "origin", "feature"]);
    run_git(&f.remote, &["update-ref", "refs/pull/7/head", &f.head]);
    run_git(&f.local, &["config", "diff.relative", "true"]);
    let nested = f.local.join("nested");
    fs::create_dir(&nested).unwrap();
    f.local = nested;
    let output = f.run(f.payload("open"), "diff");
    assert_success(&output);
    assert!(output.stdout.len() > 2_000_000);
    let patch = String::from_utf8_lossy(&output.stdout);
    assert!(
        patch.contains("GIT binary patch") && patch.contains("rename from README.md"),
        "patch omitted binary data or rename"
    );
    assert!(patch.contains("rename to renamed file.txt"));
}

#[cfg(unix)]
#[test]
fn pager_fixture() {
    use std::io::Read;
    let Some(path) = std::env::var_os("GB_SNAPSHOT_PAGER_FILE") else {
        return;
    };
    if std::env::var_os("GB_SNAPSHOT_PAGER_BLOCK").is_some() {
        fs::write(path, std::process::id().to_string()).unwrap();
        std::thread::sleep(std::time::Duration::from_secs(60));
        return;
    }
    let mut patch = Vec::new();
    std::io::stdin().read_to_end(&mut patch).unwrap();
    fs::write(path, patch).unwrap();
}

#[cfg(unix)]
#[test]
fn cancelling_a_diff_stops_a_pager_while_writing_or_waiting() {
    use std::fs::File;
    use std::os::fd::FromRawFd;
    use std::process::Stdio;
    use std::time::{Duration, Instant};

    for large in [false, true] {
        let mut f = Fixture::new(false);
        if large {
            commit_tracked_readme(
                &f.source,
                &"large patch line\n".repeat(32_768),
                "large patch",
            );
            f.head = git_output(&f.source, &["rev-parse", "HEAD"]);
            run_git(&f.source, &["push", "origin", "feature"]);
            run_git(&f.remote, &["update-ref", "refs/pull/7/head", &f.head]);
        }
        let ready = f.env.path().join("pager.pid");
        let (_master, slave) = unsafe {
            let (mut master, mut slave) = (-1, -1);
            assert_eq!(
                libc::openpty(
                    &mut master,
                    &mut slave,
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    std::ptr::null()
                ),
                0
            );
            (File::from_raw_fd(master), File::from_raw_fd(slave))
        };
        let (port, server) = spawn_scripted_server(vec![ScriptedResponse::json(
            "GET /api/v3/repos/alice/project/pulls/7 HTTP/1.1",
            "200 OK",
            &f.payload("open").to_string(),
        )]);
        let mut child = f
            .env
            .repo_api_command(format!("127.0.0.1:{port}"), "alice/project")
            .current_dir(&f.local)
            .args(["--json-errors", "pr", "diff", "7"])
            .env(
                "PAGER",
                format!(
                    "{} --exact pager_fixture --nocapture",
                    std::env::current_exe().unwrap().display()
                ),
            )
            .env("GB_SNAPSHOT_PAGER_FILE", &ready)
            .env("GB_SNAPSHOT_PAGER_BLOCK", "1")
            .stdout(slave)
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let started = Instant::now();
        while !ready.exists() && started.elapsed() < Duration::from_secs(8) {
            std::thread::sleep(Duration::from_millis(10));
        }
        if !ready.exists() {
            child.kill().unwrap();
            child.wait().unwrap();
            panic!("pager did not start");
        }
        let pager: i32 = fs::read_to_string(&ready).unwrap().parse().unwrap();
        unsafe {
            assert_eq!(libc::kill(child.id() as i32, libc::SIGTERM), 0);
        }
        let cancelled = Instant::now();
        while child.try_wait().unwrap().is_none() && cancelled.elapsed() < Duration::from_secs(5) {
            std::thread::sleep(Duration::from_millis(10));
        }
        if child.try_wait().unwrap().is_none() {
            child.kill().unwrap();
            child.wait().unwrap();
            unsafe {
                libc::kill(pager, libc::SIGKILL);
            }
            panic!("diff did not cancel while large={large}");
        }
        let output = child.wait_with_output().unwrap();
        assert_failure(&output);
        let error: Value = serde_json::from_slice(&output.stderr).unwrap();
        assert_eq!(error["error"]["code"], "cancelled", "{error}");
        assert!(error["error"].get("outcome").is_none(), "{error}");
        unsafe {
            assert_eq!(libc::kill(pager, 0), -1, "pager was not reaped");
        }
        assert_eq!(git_output(&f.local, &["branch", "--show-current"]), "main");
        server.join().unwrap();
    }
}

#[cfg(unix)]
#[test]
fn terminal_diff_uses_the_shared_pager_and_no_pager_bypasses_it() {
    use std::fs::File;
    use std::os::fd::FromRawFd;
    use std::process::Stdio;
    use std::time::{Duration, Instant};
    for no_pager in [false, true] {
        let f = Fixture::new(false);
        let recorded = f.env.path().join("pager.patch");
        let (_master, slave) = unsafe {
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
            (File::from_raw_fd(master), File::from_raw_fd(slave))
        };
        let (port, server) = spawn_scripted_server(vec![ScriptedResponse::json(
            "GET /api/v3/repos/alice/project/pulls/7 HTTP/1.1",
            "200 OK",
            &f.payload("open").to_string(),
        )]);
        let mut cmd = f
            .env
            .repo_api_command(format!("127.0.0.1:{port}"), "alice/project");
        cmd.current_dir(&f.local)
            .args(["pr", "diff", "7"])
            .env(
                "PAGER",
                format!(
                    "{} --exact pager_fixture --nocapture",
                    std::env::current_exe().unwrap().display()
                ),
            )
            .env("GB_SNAPSHOT_PAGER_FILE", &recorded)
            .stdout(slave)
            .stderr(Stdio::piped());
        if no_pager {
            cmd.arg("--no-pager");
        }
        let mut child = cmd.spawn().unwrap();
        let started = Instant::now();
        while child.try_wait().unwrap().is_none() && started.elapsed() < Duration::from_secs(8) {
            std::thread::sleep(Duration::from_millis(10));
        }
        if child.try_wait().unwrap().is_none() {
            child.kill().unwrap();
            child.wait().unwrap();
            panic!("pager input was not closed/reaped");
        }
        let output = child.wait_with_output().unwrap();
        assert_success(&output);
        assert_eq!(recorded.exists(), !no_pager);
        if !no_pager {
            assert!(fs::read_to_string(recorded)
                .unwrap()
                .contains("+original PR change"));
        }
        server.join().unwrap();
    }
}

#[test]
fn partial_or_invalid_snapshot_detail_never_activates_collection_recovery() {
    use std::io::{Read, Write};
    use std::net::TcpListener;
    for (status, body, advertised, cause) in [
        ("200 OK", "{", 100, "body_read_failed"),
        ("200 OK", "{", 1, "invalid_json"),
        ("200 OK", " ", 1, "invalid_json"),
        ("204 No Content", "", 0, "invalid_json"),
    ] {
        let env = GbTestEnv::new();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut header = Vec::new();
            while !header.ends_with(b"\r\n\r\n") {
                let mut byte = [0];
                stream.read_exact(&mut byte).unwrap();
                header.push(byte[0]);
            }
            assert!(String::from_utf8_lossy(&header)
                .starts_with("GET /api/v3/repos/alice/project/pulls/7 HTTP/1.1"));
            stream.write_all(format!("HTTP/1.1 {status}\r\nContent-Length: {advertised}\r\nConnection: close\r\n\r\n{body}").as_bytes()).unwrap();
        });
        let output = env
            .repo_api_command(format!("127.0.0.1:{port}"), "alice/project")
            .env("GB_HTTP_TIMEOUT", "2")
            .args(["--json-errors", "pr", "diff", "7", "--no-pager"])
            .output()
            .unwrap();
        assert_failure(&output);
        assert!(output.stdout.is_empty());
        let error: Value = serde_json::from_slice(&output.stderr).unwrap();
        assert_eq!(error["error"]["cause"], cause);
        assert_eq!(
            error["error"]["status"],
            if status.starts_with("204") { 204 } else { 200 }
        );
        server.join().unwrap();
    }
}

#[test]
fn retained_local_objects_remain_usable_when_fetch_sources_disappear() {
    let f = Fixture::new(false);
    assert_success(&f.run(f.payload("closed"), "diff"));
    let moved = f.env.path().join("offline.git");
    fs::rename(&f.remote, &moved).unwrap();
    let output = f.run(f.payload("closed"), "checkout");
    assert_success(&output);
    assert_eq!(git_output(&f.local, &["rev-parse", "HEAD"]), f.head);
}
