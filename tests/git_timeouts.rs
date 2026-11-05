mod support;

use std::fs;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use serde_json::Value;
use support::gb_cmd::GbTestEnv;

// Invoked by Git's fake SSH command. No remote is contacted. Both this process
// and its descendant must be terminated by the production Git supervisor.
#[test]
fn ssh_fixture() {
    let Ok(directory) = std::env::var("GB_PROCESS_FIXTURE_DIR") else {
        return;
    };
    let directory = std::path::PathBuf::from(directory);
    if std::env::var_os("GB_PROCESS_FIXTURE_CHILD").is_none() {
        fs::write(directory.join("ssh.pid"), std::process::id().to_string()).unwrap();
        let mut child = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "ssh_fixture", "--nocapture"])
            .env("GB_PROCESS_FIXTURE_CHILD", "1")
            .spawn()
            .unwrap();
        fs::write(directory.join("descendant.pid"), child.id().to_string()).unwrap();
        child.wait().unwrap();
    } else {
        thread::sleep(Duration::from_secs(60));
    }
}

fn prepare(env: &GbTestEnv) -> Command {
    let executable = std::env::current_exe()
        .unwrap()
        .to_string_lossy()
        .replace('\\', "/");
    let script = env.path().join("ssh-fixture.sh");
    fs::write(
        &script,
        format!(
            "#!/bin/sh\n'{}' --exact ssh_fixture --nocapture 2>/dev/null | while IFS= read -r line; do :; done\n",
            executable.replace('\'', "'\\''")
        ),
    )
    .unwrap();
    let script = script.to_string_lossy().replace('\\', "/");
    let mut command = env.command();
    command
        .env(
            "GIT_SSH_COMMAND",
            format!("sh '{}'", script.replace('\'', "'\\''")),
        )
        .env("GIT_SSH_VARIANT", "simple")
        .env("GB_PROCESS_FIXTURE_DIR", env.path())
        .args([
            "--json-errors",
            "repo",
            "clone",
            "ssh://git@example.invalid/project",
            "clone-result",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    command
}

fn assert_dead(pid: u32) {
    let started = Instant::now();
    while alive(pid) && started.elapsed() < Duration::from_secs(5) {
        thread::sleep(Duration::from_millis(20));
    }
    assert!(!alive(pid), "Git child {pid} survived cancellation/timeout");
}

#[cfg(unix)]
fn alive(pid: u32) -> bool {
    #[cfg(target_os = "linux")]
    if fs::read_to_string(format!("/proc/{pid}/stat"))
        .ok()
        .and_then(|s| s.rsplit_once(") ").map(|(_, state)| state.starts_with('Z')))
        .unwrap_or(false)
    {
        return false;
    }
    unsafe { libc::kill(pid as i32, 0) == 0 }
}

#[cfg(windows)]
fn alive(pid: u32) -> bool {
    use windows_sys::Win32::{
        Foundation::CloseHandle,
        System::Threading::{GetExitCodeProcess, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION},
    };
    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if handle.is_null() {
            return false;
        }
        let mut code = 0;
        let running = GetExitCodeProcess(handle, &mut code) != 0 && code == 259;
        CloseHandle(handle);
        running
    }
}

#[test]
fn git_timeout_terminates_helpers_and_descendants_and_reports_uncertainty() {
    let env = GbTestEnv::new();
    let started = Instant::now();
    let output = prepare(&env).env("GB_GIT_TIMEOUT", "2").output().unwrap();
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let error: Value = serde_json::from_slice(&output.stderr).unwrap();
    assert_eq!(error["error"]["code"], "git_timeout", "{error}");
    assert_eq!(error["error"]["cause"], "timeout");
    assert_eq!(error["error"]["outcome"], "unverified");
    assert!(started.elapsed() < Duration::from_secs(8));
    for name in ["ssh.pid", "descendant.pid"] {
        let pid = fs::read_to_string(env.path().join(name))
            .expect("SSH fixture did not start before deadline")
            .parse()
            .unwrap();
        assert_dead(pid);
    }
}

#[cfg(unix)]
#[test]
fn cancelling_cli_reaps_git_group_before_returning() {
    let env = GbTestEnv::new();
    let mut child = prepare(&env).env("GB_GIT_TIMEOUT", "20").spawn().unwrap();
    let started = Instant::now();
    while !env.path().join("descendant.pid").exists() && started.elapsed() < Duration::from_secs(8)
    {
        thread::sleep(Duration::from_millis(10));
    }
    assert!(env.path().join("descendant.pid").exists());
    unsafe {
        assert_eq!(libc::kill(child.id() as i32, libc::SIGINT), 0);
    }
    let cancel = Instant::now();
    while child.try_wait().unwrap().is_none() && cancel.elapsed() < Duration::from_secs(5) {
        thread::sleep(Duration::from_millis(10));
    }
    if child.try_wait().unwrap().is_none() {
        child.kill().unwrap();
        panic!("CLI did not finish cancellation");
    }
    let output = child.wait_with_output().unwrap();
    let error: Value = serde_json::from_slice(&output.stderr).unwrap();
    assert_eq!(error["error"]["code"], "cancelled");
    assert_eq!(error["error"]["outcome"], "unverified");
    for name in ["ssh.pid", "descendant.pid"] {
        assert_dead(
            fs::read_to_string(env.path().join(name))
                .unwrap()
                .parse()
                .unwrap(),
        );
    }
}

#[cfg(unix)]
#[test]
fn blocked_read_only_git_metadata_retains_timeout_without_claiming_a_write() {
    use std::os::unix::ffi::OsStrExt;
    let env = GbTestEnv::new();
    let path = env.path().join("blocked-config");
    let fifo = std::ffi::CString::new(path.as_os_str().as_bytes()).unwrap();
    unsafe {
        assert_eq!(libc::mkfifo(fifo.as_ptr(), 0o600), 0);
    }
    let output = env
        .repo_api_command("127.0.0.1:9", "alice/project")
        .env("GIT_CONFIG_GLOBAL", &path)
        .env("GB_GIT_TIMEOUT", "0.3")
        .args([
            "--json-errors",
            "pr",
            "create",
            "--title",
            "Title",
            "--body",
            "Body",
            "--base",
            "main",
        ])
        .output()
        .unwrap();
    assert!(!output.status.success());
    let error: Value = serde_json::from_slice(&output.stderr).unwrap();
    assert_eq!(error["error"]["code"], "git_timeout", "{error}");
    assert_eq!(error["error"]["cause"], "timeout");
    assert!(error["error"].get("outcome").is_none());
}
