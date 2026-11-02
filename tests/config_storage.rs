mod support;

use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::process::{Command, Stdio};
use std::sync::{mpsc, Arc, Barrier};
use std::thread;
use std::time::{Duration, Instant};

use support::gb_cmd::{assert_success, GbTestEnv};

fn stored(env: &GbTestEnv) -> toml::Value {
    toml::from_str(&fs::read_to_string(env.path().join("config.toml")).unwrap()).unwrap()
}

#[test]
fn concurrent_profile_updates_preserve_every_successful_change() {
    let env = GbTestEnv::new();
    let count = 12;
    let start = Arc::new(Barrier::new(count));
    let mut threads = Vec::new();
    for index in 0..count {
        let mut command = env.command();
        command.args([
            "config",
            "set",
            "profile",
            &format!("work-{index}"),
            "--default-repo",
            &format!("alice/repo-{index}"),
        ]);
        let start = Arc::clone(&start);
        threads.push(thread::spawn(move || {
            start.wait();
            let output = command.output().unwrap();
            assert_success(&output);
        }));
    }
    for thread in threads {
        thread.join().unwrap();
    }
    let config = stored(&env);
    for index in 0..count {
        assert_eq!(
            config["profiles"][format!("work-{index}")]["default_repo"].as_str(),
            Some(format!("alice/repo-{index}").as_str())
        );
    }
}

#[test]
fn login_applies_verified_credentials_to_latest_config_without_locking_during_http() {
    let env = GbTestEnv::new();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let (seen_tx, seen_rx) = mpsc::channel();
    let (resume_tx, resume_rx) = mpsc::channel();
    let server = thread::spawn(move || {
        listener.set_nonblocking(true).unwrap();
        let deadline = Instant::now() + Duration::from_secs(10);
        let mut stream = loop {
            match listener.accept() {
                Ok((stream, _)) => break stream,
                Err(err)
                    if err.kind() == std::io::ErrorKind::WouldBlock
                        && Instant::now() < deadline =>
                {
                    thread::sleep(Duration::from_millis(10))
                }
                Err(err) => panic!("login did not connect: {err}"),
            }
        };
        stream.set_nonblocking(false).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(10)))
            .unwrap();
        stream
            .set_write_timeout(Some(Duration::from_secs(10)))
            .unwrap();
        let mut reader = BufReader::new(&mut stream);
        let mut line = String::new();
        loop {
            line.clear();
            assert!(reader.read_line(&mut line).unwrap() > 0);
            if line == "\r\n" {
                break;
            }
        }
        seen_tx.send(()).unwrap();
        resume_rx.recv_timeout(Duration::from_secs(10)).unwrap();
        let body = r#"{"login":"alice"}"#;
        write!(
            stream,
            "HTTP/1.1 200 OK\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
            body.len()
        )
        .unwrap();
    });
    let child = env
        .command()
        .args([
            "auth",
            "login",
            "-H",
            &format!("http://127.0.0.1:{port}"),
            "-t",
            "dummy-login-token",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    seen_rx.recv_timeout(Duration::from_secs(10)).unwrap();
    let changed = env
        .command()
        .args([
            "config",
            "set",
            "profile",
            "concurrent",
            "--default-repo",
            "alice/keep-me",
        ])
        .output()
        .unwrap();
    // Unblock the HTTP request even if the assertion below fails.
    resume_tx.send(()).unwrap();
    assert_success(&changed);
    let output = child.wait_with_output().unwrap();
    server.join().unwrap();
    assert_success(&output);
    let config = stored(&env);
    assert_eq!(
        config["profiles"]["concurrent"]["default_repo"].as_str(),
        Some("alice/keep-me")
    );
    assert_eq!(
        config["hosts"][format!("http://127.0.0.1:{port}")]["user"].as_str(),
        Some("alice")
    );
}

#[test]
fn readers_only_observe_complete_configs_during_repeated_updates() {
    let env = GbTestEnv::new();
    let path = env.path().join("config.toml");
    fs::write(&path, "[profiles.seed]\ndefault_repo = 'alice/seed'\n").unwrap();
    let config_dir = env.path().to_path_buf();
    let (ready_tx, ready_rx) = mpsc::channel();
    let writer = thread::spawn(move || {
        ready_tx.send(()).unwrap();
        for index in 0..30 {
            let output = support::gb_cmd::gb_command()
                .env("GB_CONFIG_DIR", &config_dir)
                .args([
                    "config",
                    "set",
                    "profile",
                    "writer",
                    "--default-repo",
                    &format!("alice/repo-{index}"),
                ])
                .output()
                .unwrap();
            assert_success(&output);
        }
    });
    // Read from the independently owned path while the writer replaces it.
    ready_rx.recv().unwrap();
    let path = env.path().join("config.toml");
    let mut observed = 0;
    while !writer.is_finished() {
        let config: toml::Value = toml::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(
            config["profiles"]["seed"]["default_repo"].as_str(),
            Some("alice/seed")
        );
        observed += 1;
        thread::yield_now();
    }
    writer.join().unwrap();
    assert!(observed > 0);
    assert_eq!(
        stored(&env)["profiles"]["writer"]["default_repo"].as_str(),
        Some("alice/repo-29")
    );
}

// Runs only as an explicitly selected subprocess of the release test below.
#[test]
fn lock_holder_child() {
    let Some(path) = std::env::var_os("GB_TEST_LOCK_PATH") else {
        return;
    };
    let lock = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)
        .unwrap();
    fs2::FileExt::lock_exclusive(&lock).unwrap();
    println!("GB_TEST_LOCK_READY");
    std::io::stdout().flush().unwrap();
    loop {
        thread::park();
    }
}

#[test]
fn process_exit_releases_lock_without_removing_lock_file() {
    let env = GbTestEnv::new();
    let path = env.path().join("config.toml.lock");
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "lock_holder_child", "--nocapture"])
        .env("GB_TEST_LOCK_PATH", &path)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let stdout = child.stdout.take().unwrap();
    let (ready_tx, ready_rx) = mpsc::channel();
    let reader = thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            if line.unwrap().contains("GB_TEST_LOCK_READY") {
                ready_tx.send(()).unwrap();
                break;
            }
        }
    });
    if ready_rx.recv_timeout(Duration::from_secs(10)).is_err() {
        let _ = child.kill();
        let _ = child.wait();
        panic!("lock holder did not start");
    }
    // A live OS lock cannot be taken by an independent handle.
    let lock = OpenOptions::new()
        .read(true)
        .write(true)
        .open(&path)
        .unwrap();
    let contention = fs2::FileExt::try_lock_exclusive(&lock);
    drop(lock);
    child.kill().unwrap();
    child.wait().unwrap();
    reader.join().unwrap();
    assert_eq!(
        contention.unwrap_err().raw_os_error(),
        fs2::lock_contended_error().raw_os_error()
    );
    assert!(path.is_file());
    let output = env
        .command()
        .args(["config", "set", "profile", "after-exit"])
        .output()
        .unwrap();
    assert_success(&output);
    assert!(stored(&env)["profiles"].get("after-exit").is_some());
}

#[test]
fn malformed_config_is_not_replaced_by_a_mutation() {
    let env = GbTestEnv::new();
    let path = env.path().join("config.toml");
    fs::write(&path, "[broken\n").unwrap();
    let output = env
        .command()
        .args(["config", "set", "profile", "must-not-appear"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert_eq!(fs::read_to_string(path).unwrap(), "[broken\n");
}

#[test]
fn held_lock_times_out_without_writing_or_printing_success() {
    let env = GbTestEnv::new();
    let path = env.path().join("config.toml");
    let original = "# preserve on timeout\n[profiles.seed]\n";
    fs::write(&path, original).unwrap();
    let lock = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(env.path().join("config.toml.lock"))
        .unwrap();
    fs2::FileExt::lock_exclusive(&lock).unwrap();
    let output = env
        .command()
        .args(["config", "set", "profile", "blocked"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("Configuration is busy"));
    assert_eq!(fs::read_to_string(&path).unwrap(), original);
    drop(lock);
    let output = env
        .command()
        .args(["config", "set", "profile", "blocked"])
        .output()
        .unwrap();
    assert_success(&output);
    assert!(stored(&env)["profiles"].get("blocked").is_some());
}
