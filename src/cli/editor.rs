//! Explicit, terminal-owned editor input. No shell or implicit launch.
use crate::error::{GbError, Result};
use process_wrap::std::{ChildWrapper, CommandWrap};
use std::{
    fs::{File, OpenOptions},
    io::{Read, Write},
    path::Path,
    process::{Command, Stdio},
    time::Duration,
};

fn failure(cause: &'static str, message: &str) -> GbError {
    GbError::Editor {
        cause,
        status: None,
        message: message.into(),
    }
}

fn selected_command() -> Result<Command> {
    selected_from(|name| std::env::var_os(name))
}

fn selected_from(lookup: impl Fn(&str) -> Option<std::ffi::OsString>) -> Result<Command> {
    for name in ["GB_EDITOR", "VISUAL", "EDITOR"] {
        if let Some(value) = lookup(name) {
            let value = value
                .into_string()
                .map_err(|_| failure("invalid_command", "Editor setting must be UTF-8."))?;
            return parse_command(&value);
        }
    }
    parse_command(if cfg!(windows) { "notepad.exe" } else { "vi" })
}

fn parse_command(value: &str) -> Result<Command> {
    let parts = shell_words::split(value)
        .map_err(|_| failure("invalid_command", "Editor setting has invalid quoting."))?;
    let (program, args) = parts
        .split_first()
        .filter(|(program, _)| !program.is_empty())
        .ok_or_else(|| failure("invalid_command", "Editor setting is empty."))?;
    let mut command = Command::new(program);
    command.args(args);
    Ok(command)
}

#[cfg(unix)]
struct TerminalState(Vec<(std::os::fd::OwnedFd, libc::termios)>);

#[cfg(unix)]
impl TerminalState {
    fn capture() -> std::io::Result<Self> {
        use std::os::fd::{AsFd, AsRawFd};
        let stdin = std::io::stdin();
        let stderr = std::io::stderr();
        let mut state = Self(Vec::new());
        for handle in [stdin.as_fd(), stderr.as_fd()] {
            let handle = handle.try_clone_to_owned()?;
            let mut settings = std::mem::MaybeUninit::uninit();
            loop {
                if unsafe { libc::tcgetattr(handle.as_raw_fd(), settings.as_mut_ptr()) } == 0 {
                    state.0.push((handle, unsafe { settings.assume_init() }));
                    break;
                }
                let error = std::io::Error::last_os_error();
                if error.raw_os_error() == Some(libc::ENOTTY) {
                    break; // Nonterminal unit-test streams have no modes to save.
                }
                if error.kind() != std::io::ErrorKind::Interrupted {
                    return Err(error);
                }
            }
        }
        Ok(state)
    }
}

#[cfg(unix)]
impl Drop for TerminalState {
    fn drop(&mut self) {
        use std::os::fd::AsRawFd;
        for (handle, settings) in &self.0 {
            loop {
                if unsafe { libc::tcsetattr(handle.as_raw_fd(), libc::TCSANOW, settings) } == 0
                    || std::io::Error::last_os_error().kind() != std::io::ErrorKind::Interrupted
                {
                    break;
                }
            }
        }
    }
}

struct Child(Box<dyn ChildWrapper>, bool);

impl Drop for Child {
    fn drop(&mut self) {
        // Do not signal a reaped Unix PID: it can already have been reused.
        // Windows signals the owned job, never an unrelated reused process.
        if self.1 || cfg!(windows) {
            let _ = self.0.start_kill();
        }
        if self.1 {
            // Only JobObject wraps this native child on Windows. Reap that
            // child directly: JobObject::try_wait consumes completion events,
            // so a later JobObject::wait can block on an already consumed one.
            #[cfg(windows)]
            let _ = self.0.inner_mut().wait();
            #[cfg(not(windows))]
            let _ = self.0.wait();
        }
    }
}

pub(super) fn edit(seed: &str) -> Result<String> {
    edit_with(seed, selected_command()?, crate::cancel::requested)
}

fn edit_with(seed: &str, mut command: Command, cancelled: impl Fn() -> bool) -> Result<String> {
    if cancelled() {
        return Err(GbError::Cancelled);
    }
    let mut builder = tempfile::Builder::new();
    builder.prefix("gb-editor-");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        builder.permissions(std::fs::Permissions::from_mode(0o700));
    }
    let directory = builder.tempdir()?;
    let path = directory.path().join("body.md");
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    {
        let mut file = options.open(&path)?;
        file.write_all(seed.as_bytes())?;
        file.flush()?;
    } // Close before launching so Windows editors can replace the file.
    #[cfg(unix)]
    let terminal_output = {
        use std::os::fd::AsFd;
        std::io::stderr().as_fd().try_clone_to_owned()?
    };
    #[cfg(windows)]
    let terminal_output = {
        use std::os::windows::io::AsHandle;
        std::io::stderr().as_handle().try_clone_to_owned()?
    };
    command
        .arg(&path)
        .stdin(Stdio::inherit())
        .stdout(Stdio::from(terminal_output))
        .stderr(Stdio::inherit());
    let mut command = CommandWrap::from(command);
    // Unix editors stay in the foreground terminal group, like the pager.
    #[cfg(windows)]
    command.wrap(process_wrap::std::JobObject);
    // Declare before Child so early returns kill/reap it before restoring modes.
    // Editors killed during cancellation cannot run their own terminal cleanup.
    #[cfg(unix)]
    let terminal_state = TerminalState::capture()?;
    let mut child = Child(
        command
            .spawn()
            .map_err(|_| failure("spawn_failed", "Could not start the selected editor."))?,
        true,
    );
    let status = loop {
        if cancelled() {
            return Err(GbError::Cancelled);
        }
        if let Some(status) = child.0.try_wait()? {
            child.1 = false;
            break status;
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    if cancelled() {
        return Err(GbError::Cancelled);
    }
    if !status.success() {
        return Err(GbError::Editor {
            cause: "nonzero_exit",
            status: status.code(),
            message: "The editor exited unsuccessfully; no body was submitted.".into(),
        });
    }
    drop(child);
    #[cfg(unix)]
    drop(terminal_state);
    let mut file = open_body(&path)?;
    let mut text = String::new();
    file.read_to_string(&mut text)
        .map_err(|_| failure("invalid_body", "The edited body must be readable UTF-8."))?;
    drop(file);
    directory.close()?;
    if cancelled() || text == seed {
        return Err(GbError::Cancelled);
    }
    Ok(text)
}

fn regular(metadata: &std::fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if metadata.file_attributes() & 0x400 != 0 {
            return false;
        }
    }
    metadata.is_file() && !metadata.file_type().is_symlink()
}

fn open_body(path: &Path) -> Result<File> {
    let unreadable = || {
        failure(
            "invalid_body",
            "The edited body must be a readable regular file.",
        )
    };
    if !regular(&std::fs::symlink_metadata(path).map_err(|_| unreadable())?) {
        return Err(failure(
            "invalid_body",
            "The editor must leave a regular body file, not a symlink or special file.",
        ));
    }
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        options.custom_flags(0x00200000); // FILE_FLAG_OPEN_REPARSE_POINT
    }
    let file = options.open(path).map_err(|_| unreadable())?;
    if !regular(&file.metadata().map_err(|_| unreadable())?) {
        return Err(failure(
            "invalid_body",
            "The edited body is not a regular file.",
        ));
    }
    Ok(file)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    #[test]
    fn editor_fixture() {
        let Some(mode) = std::env::var_os("GB_EDITOR_FIXTURE_MODE") else {
            return;
        };
        let path = std::path::PathBuf::from(std::env::args_os().last().unwrap());
        let log = std::path::PathBuf::from(std::env::var_os("GB_EDITOR_FIXTURE_LOG").unwrap());
        std::fs::write(&log, path.to_str().unwrap()).unwrap();
        std::fs::copy(&path, log.with_extension("seed")).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                0o600
            );
            assert_eq!(
                std::fs::metadata(path.parent().unwrap())
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777,
                0o700
            );
        }
        match mode.to_str().unwrap() {
            "unchanged" => {}
            "empty" => std::fs::write(&path, "").unwrap(),
            "exit" => std::process::exit(7),
            "invalid" => std::fs::write(&path, [0xff]).unwrap(),
            "missing" => std::fs::remove_file(&path).unwrap(),
            "directory" => {
                std::fs::remove_file(&path).unwrap();
                std::fs::create_dir(&path).unwrap();
            }
            #[cfg(unix)]
            "symlink" => {
                std::fs::remove_file(&path).unwrap();
                std::os::unix::fs::symlink(log.with_extension("seed"), &path).unwrap();
            }
            "sleep" => std::thread::sleep(Duration::from_secs(60)),
            "atomic" => {
                let replacement = path.with_extension("replacement");
                std::fs::write(&replacement, "edited 日本語\r\n  tail\n").unwrap();
                // Closing the initial file permits this on Windows too.
                std::fs::remove_file(&path).unwrap();
                std::fs::rename(replacement, &path).unwrap();
            }
            _ => panic!("unknown fixture mode"),
        }
    }

    fn fixture(mode: &str) -> (tempfile::TempDir, Command) {
        let dir = tempfile::tempdir().unwrap();
        let mut command = Command::new(std::env::current_exe().unwrap());
        command
            .args([
                "--exact",
                "cli::editor::tests::editor_fixture",
                "--nocapture",
                "--quiet",
            ])
            .env("GB_EDITOR_FIXTURE_MODE", mode)
            .env("GB_EDITOR_FIXTURE_LOG", dir.path().join("path"));
        (dir, command)
    }

    fn cleaned(dir: &Path) {
        let body = std::fs::read_to_string(dir.join("path")).unwrap();
        assert!(
            !Path::new(&body).parent().unwrap().exists(),
            "editor directory was not removed"
        );
    }

    #[test]
    fn editor_selection_quotes_arguments_and_never_uses_a_shell() {
        let command =
            parse_command("'editor with spaces' --wait 'argument with spaces' ';' '$HOME'")
                .unwrap();
        assert_eq!(command.get_program(), "editor with spaces");
        assert_eq!(
            command.get_args().collect::<Vec<_>>(),
            ["--wait", "argument with spaces", ";", "$HOME"]
        );
        for invalid in ["", " ", "''", "'unterminated"] {
            assert!(parse_command(invalid).is_err());
        }
        for selected in ["GB_EDITOR", "VISUAL", "EDITOR"] {
            let command =
                selected_from(|key| (key == selected).then(|| "chosen --wait".into())).unwrap();
            assert_eq!(command.get_program(), "chosen");
        }
        let command = selected_from(|key| Some(key.into())).unwrap();
        assert_eq!(command.get_program(), "GB_EDITOR");
        let command = selected_from(|key| (key != "GB_EDITOR").then(|| key.into())).unwrap();
        assert_eq!(command.get_program(), "VISUAL");
        assert!(
            selected_from(|key| Some(if key == "GB_EDITOR" { "" } else { "other" }.into()))
                .is_err()
        );
    }

    #[test]
    fn editor_preserves_seed_and_replacement_bytes_and_removes_private_files() {
        let (dir, command) = fixture("atomic");
        let text = edit_with("original\r\nseed\n", command, || false).unwrap();
        assert_eq!(text, "edited 日本語\r\n  tail\n");
        assert_eq!(
            std::fs::read_to_string(dir.path().join("path.seed")).unwrap(),
            "original\r\nseed\n"
        );
        cleaned(dir.path());
    }

    #[test]
    fn editor_unchanged_exit_failure_invalid_utf8_and_nonregular_files_never_return_a_body() {
        for mode in ["unchanged", "exit", "invalid", "directory", "missing"] {
            let (dir, command) = fixture(mode);
            let error = edit_with("private seed", command, || false).unwrap_err();
            assert!(!error.to_string().contains("private seed"));
            if mode == "unchanged" {
                assert_eq!(error.code(), "cancelled");
            } else {
                assert_eq!(error.code(), "editor_failed", "mode: {mode}");
            }
            if mode == "exit" {
                assert_eq!(error.child_exit_code(), Some(7));
            }
            cleaned(dir.path());
        }
        let error = edit_with(
            "private seed",
            Command::new("gb-nonexistent-editor-for-test"),
            || false,
        )
        .unwrap_err();
        assert_eq!(error.cause_code(), Some("spawn_failed"));
    }

    #[test]
    fn editor_can_clear_an_existing_description() {
        let (dir, command) = fixture("empty");
        assert_eq!(edit_with("old", command, || false).unwrap(), "");
        cleaned(dir.path());
    }

    #[test]
    fn cancellation_stops_and_reaps_the_editor_before_removing_its_file() {
        let (dir, command) = fixture("sleep");
        let start = Instant::now();
        let ready = dir.path().join("path");
        let error = edit_with("seed", command, || {
            std::fs::metadata(&ready).is_ok_and(|metadata| metadata.len() > 0)
                || start.elapsed() > Duration::from_secs(5)
        })
        .unwrap_err();
        assert_eq!(error.code(), "cancelled");
        assert!(start.elapsed() < Duration::from_secs(5));
        cleaned(dir.path());
    }

    #[cfg(unix)]
    #[test]
    fn editor_rejects_symlinks_without_reading_or_removing_their_target() {
        let (dir, command) = fixture("symlink");
        assert_eq!(
            edit_with("private target", command, || false)
                .unwrap_err()
                .cause_code(),
            Some("invalid_body")
        );
        cleaned(dir.path());
        assert_eq!(
            std::fs::read_to_string(dir.path().join("path.seed")).unwrap(),
            "private target"
        );
    }
}
