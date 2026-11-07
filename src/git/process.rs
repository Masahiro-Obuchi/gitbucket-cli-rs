//! Supervise Git and its process group/job, including inherited output pipes.
use std::io::{self, Read};
use std::process::{Command, Output, Stdio};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use process_wrap::std::{ChildWrapper, CommandWrap};

use crate::error::{GbError, Result};

pub(crate) trait GitCommandExt {
    /// Consumes this command's configuration, like a one-shot invocation.
    fn bounded_output(&mut self) -> Result<Output>;
    fn bounded_output_to_file(&mut self, stdout: std::fs::File) -> Result<Output>;
}

impl GitCommandExt for Command {
    fn bounded_output(&mut self) -> Result<Output> {
        let timeout = crate::limits::duration("GB_GIT_TIMEOUT", 300)?;
        let program = self.get_program().to_os_string();
        let command = std::mem::replace(self, Command::new(program));
        run(command, timeout, None)
    }

    fn bounded_output_to_file(&mut self, stdout: std::fs::File) -> Result<Output> {
        let timeout = crate::limits::duration("GB_GIT_TIMEOUT", 300)?;
        let program = self.get_program().to_os_string();
        let command = std::mem::replace(self, Command::new(program));
        run(command, timeout, Some(stdout))
    }
}

struct Guard(Box<dyn ChildWrapper>, bool);
impl Guard {
    fn terminate(&mut self) -> io::Result<()> {
        let killed = self.0.start_kill();
        // Reap even if signalling raced with natural exit. Never leave a live
        // child behind merely because a reader, wait or cancellation failed.
        let waited = self.0.wait();
        self.1 = false;
        waited?;
        if let Err(error) = killed {
            if error.kind() != io::ErrorKind::NotFound && error.raw_os_error() != Some(3) {
                return Err(error);
            }
        }
        Ok(())
    }
}
impl Drop for Guard {
    fn drop(&mut self) {
        if self.1 {
            let _ = self.terminate();
        }
    }
}

fn reader(pipe: impl Read + Send + 'static) -> io::Result<JoinHandle<io::Result<Vec<u8>>>> {
    thread::Builder::new()
        .name("gb-git-output".into())
        .spawn(move || {
            let mut pipe = pipe;
            let mut bytes = Vec::new();
            pipe.read_to_end(&mut bytes)?;
            Ok(bytes)
        })
}

fn run(
    mut command: Command,
    timeout: Duration,
    stdout_file: Option<std::fs::File>,
) -> Result<Output> {
    let operation = [
        "clone",
        "fetch",
        "checkout",
        "diff",
        "status",
        "remote",
        "config",
        "ls-remote",
        "symbolic-ref",
        "branch",
        "for-each-ref",
        "check-ref-format",
        "rev-parse",
        "update-ref",
    ]
    .into_iter()
    .find(|name| command.get_args().any(|arg| arg == *name))
    .unwrap_or("command");
    let mutating = match operation {
        "clone" | "fetch" | "checkout" | "update-ref" => true,
        "config" => !command.get_args().any(|arg| {
            matches!(
                arg.to_str(),
                Some("--get" | "--get-all" | "--list" | "--get-regexp")
            )
        }),
        "branch" => !command.get_args().any(|arg| arg == "--show-current"),
        "command" => true,
        _ => false,
    };
    let failure = |reason, source| GbError::Git {
        operation,
        reason,
        source,
        mutating: mutating && reason != "spawn failed",
    };
    if crate::cancel::requested() {
        return Err(failure("cancelled", None));
    }
    command
        .stdin(Stdio::null())
        .stderr(Stdio::piped())
        .env("GIT_TERMINAL_PROMPT", "0");
    command.stdout(match stdout_file {
        Some(file) => Stdio::from(file),
        None => Stdio::piped(),
    });
    let mut command = CommandWrap::from(command);
    #[cfg(unix)]
    command.wrap(process_wrap::std::ProcessGroup::leader());
    #[cfg(windows)]
    command.wrap(process_wrap::std::JobObject);
    let started = Instant::now();
    let mut child = Guard(
        command
            .spawn()
            .map_err(|error| failure("spawn failed", Some(error)))?,
        true,
    );
    let stdout = match child.0.stdout().take() {
        Some(pipe) => reader(pipe),
        None => reader(io::empty()),
    }
    .map_err(|error| failure("output reader failed", Some(error)))?;
    let stderr = reader(child.0.stderr().take().expect("piped stderr"))
        .map_err(|error| failure("output reader failed", Some(error)))?;
    let status = loop {
        if crate::cancel::requested() || started.elapsed() >= timeout {
            let reason = if crate::cancel::requested() {
                "cancelled"
            } else {
                "timed out"
            };
            child
                .terminate()
                .map_err(|error| failure("cleanup failed", Some(error)))?;
            // Closing the group also closes inherited pipes in hooks/helpers.
            // Do not let an escaped daemon holding a pipe defeat the deadline.
            let grace = Instant::now();
            while !(stdout.is_finished() && stderr.is_finished())
                && grace.elapsed() < Duration::from_secs(1)
            {
                thread::sleep(Duration::from_millis(10));
            }
            return Err(failure(reason, None));
        }
        if let Some(status) = child
            .0
            .try_wait()
            .map_err(|error| failure("wait failed", Some(error)))?
        {
            if stdout.is_finished() && stderr.is_finished() {
                break status;
            }
        }
        thread::sleep(Duration::from_millis(10));
    };
    child.1 = false;
    let join = |handle: JoinHandle<io::Result<Vec<u8>>>| {
        handle
            .join()
            .map_err(|_| failure("output reader panicked", None))?
            .map_err(|error| failure("output read failed", Some(error)))
    };
    Ok(Output {
        status,
        stdout: join(stdout)?,
        stderr: join(stderr)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pipe_fixture() {
        let Ok(path) = std::env::var("GB_PIPE_FIXTURE") else {
            return;
        };
        if std::env::var_os("GB_PIPE_CHILD").is_some() {
            thread::sleep(Duration::from_secs(60));
            return;
        }
        let child = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "git::process::tests::pipe_fixture",
                "--nocapture",
            ])
            .env("GB_PIPE_CHILD", "1")
            .spawn()
            .unwrap();
        std::fs::write(path, child.id().to_string()).unwrap();
        // Intentionally exit before this child: it retains our output pipes.
        drop(child);
    }

    #[test]
    fn inherited_pipes_do_not_defeat_deadline_after_leader_exit() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("child.pid");
        let mut command = Command::new(std::env::current_exe().unwrap());
        command
            .args([
                "--exact",
                "git::process::tests::pipe_fixture",
                "--nocapture",
            ])
            .env("GB_PIPE_FIXTURE", &path);
        let started = Instant::now();
        let result = run(command, Duration::from_secs(2), None);
        assert!(
            matches!(
                result,
                Err(GbError::Git {
                    reason: "timed out",
                    ..
                })
            ),
            "{result:?}"
        );
        assert!(path.exists(), "fixture did not start");
        assert!(started.elapsed() < Duration::from_secs(5));
    }
}
