#![cfg(unix)]

mod support;

use std::fs::File;
use std::io::{Read, Write};
use std::os::fd::{AsRawFd, FromRawFd};
use std::process::{Child, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use support::gb_cmd::GbTestEnv;

fn wait_for_exit(child: &mut Child) {
    let started = Instant::now();
    while child.try_wait().unwrap().is_none() && started.elapsed() < Duration::from_secs(5) {
        thread::sleep(Duration::from_millis(10));
    }
    if child.try_wait().unwrap().is_none() {
        child.kill().unwrap();
        child.wait().unwrap();
        panic!("CLI ignored cancellation while waiting for input");
    }
}

#[test]
fn signals_and_ctrl_c_cancel_hostname_and_password_prompts() {
    for (password, signal) in [
        (false, Some(libc::SIGTERM)),
        (true, Some(libc::SIGTERM)),
        (false, None),
    ] {
        let env = GbTestEnv::new();
        let (mut master, slave) = unsafe {
            let (mut master, mut slave) = (-1, -1);
            assert_eq!(
                libc::openpty(
                    &mut master,
                    &mut slave,
                    std::ptr::null_mut(),
                    std::ptr::null(),
                    std::ptr::null(),
                ),
                0
            );
            let flags = libc::fcntl(master, libc::F_GETFL);
            assert_ne!(flags, -1);
            assert_ne!(
                libc::fcntl(master, libc::F_SETFL, flags | libc::O_NONBLOCK),
                -1
            );
            (File::from_raw_fd(master), File::from_raw_fd(slave))
        };
        let mut command = env.command();
        command.args(["--json-errors", "auth", "login"]);
        if password {
            command.args(["--hostname", "example.invalid"]);
        }
        let mut child = command
            .stdin(slave.try_clone().unwrap())
            .stderr(slave)
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        let expected = if password {
            "Personal access token"
        } else {
            "GitBucket host or URL"
        };
        let started = Instant::now();
        let mut output = String::new();
        while !output.contains(expected) && started.elapsed() < Duration::from_secs(8) {
            let mut bytes = [0; 2048];
            match master.read(&mut bytes) {
                Ok(count) => output.push_str(&String::from_utf8_lossy(&bytes[..count])),
                Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {}
                Err(err) => panic!("cannot read prompt: {err}; {output}"),
            }
            thread::sleep(Duration::from_millis(10));
        }
        if !output.contains(expected) {
            child.kill().unwrap();
            child.wait().unwrap();
            panic!("prompt did not start: {output}");
        }
        if let Some(signal) = signal {
            unsafe {
                assert_eq!(libc::kill(child.id() as i32, signal), 0);
            }
        } else {
            master.write_all(b"\x03").unwrap();
        }
        wait_for_exit(&mut child);
        let mut bytes = [0; 4096];
        while let Ok(count) = master.read(&mut bytes) {
            if count == 0 {
                break;
            }
            output.push_str(&String::from_utf8_lossy(&bytes[..count]));
        }
        assert!(output.contains("\"code\":\"cancelled\""), "{output}");
        assert!(!output.contains("\"outcome\""), "{output}");
        assert!(!child.wait().unwrap().success());
    }
}

#[test]
fn sigterm_cancels_a_pending_stdin_json_read() {
    let env = GbTestEnv::new();
    let (input, mut writer) = unsafe {
        let mut fds = [-1; 2];
        assert_eq!(libc::pipe(fds.as_mut_ptr()), 0);
        (File::from_raw_fd(fds[0]), File::from_raw_fd(fds[1]))
    };
    let mut child = env
        .api_command("127.0.0.1:9")
        .args(["--json-errors", "api", "user", "--input", "-"])
        .stdin(input.try_clone().unwrap())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    writer.write_all(b" ").unwrap();
    // Wait until the command has consumed a byte and is blocked on more stdin,
    // rather than racing its startup with an arbitrary sleep.
    let started = Instant::now();
    loop {
        let mut unread: libc::c_int = 0;
        unsafe {
            assert_ne!(
                libc::ioctl(input.as_raw_fd(), libc::FIONREAD, &mut unread),
                -1
            );
        }
        if unread == 0 {
            break;
        }
        if started.elapsed() > Duration::from_secs(8) {
            child.kill().unwrap();
            child.wait().unwrap();
            panic!("CLI did not begin reading stdin");
        }
        thread::sleep(Duration::from_millis(10));
    }
    unsafe {
        assert_eq!(libc::kill(child.id() as i32, libc::SIGTERM), 0);
    }
    wait_for_exit(&mut child);
    let output = child.wait_with_output().unwrap();
    let error: serde_json::Value = serde_json::from_slice(&output.stderr).unwrap();
    assert_eq!(error["error"]["code"], "cancelled");
    assert_eq!(error["error"]["cause"], "cancelled");
    assert!(error["error"].get("outcome").is_none());
    assert!(output.stdout.is_empty());
}
