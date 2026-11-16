#![allow(dead_code)]
//! Terminal-backed CLI/editor fixtures; all data is disposable.
use std::{
    fs::File,
    io::Read,
    os::fd::FromRawFd,
    path::Path,
    process::{Command, Output, Stdio},
    time::{Duration, Instant},
};

pub fn fixture() {
    let Some(mode) = std::env::var_os("TEST_EDITOR_MODE") else {
        return;
    };
    let path = std::path::PathBuf::from(std::env::args_os().last().unwrap());
    let log = std::path::PathBuf::from(std::env::var_os("TEST_EDITOR_LOG").unwrap());
    std::fs::write(&log, path.to_str().unwrap()).unwrap();
    std::fs::copy(&path, log.with_extension("seed")).unwrap();
    std::println!("EDITOR-STDOUT-MARKER");
    match mode.to_str().unwrap() {
        "write" => std::fs::write(path, std::env::var("TEST_EDITOR_TEXT").unwrap()).unwrap(),
        "unchanged" => {}
        "fail" => std::process::exit(9),
        "invalid" => std::fs::write(path, [0xff]).unwrap(),
        "sleep" => std::thread::sleep(Duration::from_secs(60)),
        _ => panic!("unknown editor fixture mode"),
    }
}

pub fn configure(command: &mut Command, log: &Path, text: &str, mode: &str) {
    let executable = std::env::current_exe().unwrap();
    let value = shell_words::join([
        executable.to_str().unwrap(),
        "--exact",
        "editor_fixture",
        "--nocapture",
        "--quiet",
    ]);
    command
        .env_remove("GB_JSON_ERRORS")
        .env("GB_EDITOR", value)
        .env("VISUAL", "gb-unselected-editor")
        .env("TEST_EDITOR_LOG", log)
        .env("TEST_EDITOR_TEXT", text)
        .env("TEST_EDITOR_MODE", mode);
}

pub fn run(command: &mut Command) -> Output {
    run_cancel(command, None)
}

pub fn run_cancel(command: &mut Command, ready: Option<&Path>) -> Output {
    let (mut master, slave) = unsafe {
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
        (File::from_raw_fd(master), File::from_raw_fd(slave))
    };
    let mut child = command
        .stdin(slave.try_clone().unwrap())
        .stderr(slave)
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let start = Instant::now();
    let mut terminal = Vec::new();
    let mut cancelled = false;
    loop {
        let mut bytes = [0; 4096];
        if let Ok(count) = master.read(&mut bytes) {
            terminal.extend_from_slice(&bytes[..count]);
        }
        if !cancelled
            && ready.is_some_and(|path| {
                std::fs::metadata(path).is_ok_and(|metadata| metadata.len() > 0)
            })
        {
            assert_eq!(unsafe { libc::kill(child.id() as i32, libc::SIGTERM) }, 0);
            cancelled = true;
        }
        if child.try_wait().unwrap().is_some() {
            break;
        }
        if start.elapsed() > Duration::from_secs(12) {
            let _ = child.kill();
            let _ = child.wait();
            panic!(
                "editor CLI timed out: {}",
                String::from_utf8_lossy(&terminal)
            );
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    loop {
        let mut bytes = [0; 4096];
        match master.read(&mut bytes) {
            Ok(0) | Err(_) => break,
            Ok(count) => terminal.extend_from_slice(&bytes[..count]),
        }
    }
    let mut output = child.wait_with_output().unwrap();
    output.stderr = terminal;
    output
}

pub fn assert_cleaned(log: &Path) {
    let path = std::fs::read_to_string(log).unwrap();
    assert!(!Path::new(&path).parent().unwrap().exists());
}
