pub(crate) mod comments;
mod error;
pub(crate) use error::{argument_error, report_error};
mod stdout;
mod text;
pub use stdout::{finish, stdout_line, stdout_write, write_stdout};
pub mod table;

use colored::Colorize;
use serde::Serialize;
use std::io::{IsTerminal, Read, Write};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};

use crate::error::{GbError, Result};

static SUPPRESS_STDERR: AtomicBool = AtomicBool::new(false);

pub fn set_suppress_stderr(suppress: bool) {
    SUPPRESS_STDERR.store(suppress, Ordering::Relaxed);
}

pub fn suppress_stderr() -> bool {
    SUPPRESS_STDERR.load(Ordering::Relaxed)
}

/// Format a scalar human field without allowing it to add lines or columns.
pub fn single_line(text: &str) -> String {
    self::text::cell(text)
}

pub fn stderr_line(args: std::fmt::Arguments<'_>) {
    if suppress_stderr() {
        return;
    }
    let mut stderr = std::io::stderr().lock();
    let _ = stderr.write_all(text::human(&args.to_string()).as_bytes());
    let _ = stderr.write_all(b"\n");
}

pub fn stderr_write(args: std::fmt::Arguments<'_>) {
    if suppress_stderr() {
        return;
    }
    let mut stderr = std::io::stderr().lock();
    let _ = stderr.write_all(text::human(&args.to_string()).as_bytes());
}

pub fn print_json<T: Serialize>(value: &T) -> Result<()> {
    write_stdout(format!("{}\n", serde_json::to_string_pretty(value)?).as_bytes())
}

pub fn page_json<T: Serialize>(value: &T, no_pager: bool) -> Result<()> {
    page_reader(
        &mut format!("{}\n", serde_json::to_string_pretty(value)?).as_bytes(),
        no_pager,
    )?;
    Ok(())
}

pub fn open_web_url(url: &str) -> Result<()> {
    open::that(url).map_err(|err| GbError::Other(format!("Failed to open browser: {}", err)))?;
    println!("Opening {} in your browser.", url);
    Ok(())
}

/// Format a state label with color
pub fn format_state(state: &str) -> String {
    match state.to_lowercase().as_str() {
        "open" => "OPEN".green().bold().to_string(),
        "closed" => "CLOSED".red().bold().to_string(),
        "draft" => "DRAFT".cyan().bold().to_string(),
        "merged" => "MERGED".magenta().bold().to_string(),
        _ => single_line(state),
    }
}

/// Truncate a string to a max width, appending "..." if needed
pub fn truncate(s: &str, max: usize) -> String {
    text::truncate(s, max)
}

pub fn page_or_print(content: &str, no_pager: bool) -> Result<()> {
    page_reader(&mut text::human(content).as_bytes(), no_pager)
}

/// Stream validated output, closing stdin and reaping even an early-exit pager.
pub fn page_reader(content: &mut impl Read, no_pager: bool) -> Result<()> {
    if crate::cancel::requested() {
        return Err(GbError::Cancelled);
    }
    if no_pager || !std::io::stdout().is_terminal() {
        return stdout::stream_stdout(content);
    }

    let pager = std::env::var("PAGER").unwrap_or_else(|_| "less -R".to_string());
    let mut parts = pager.split_whitespace();
    let Some(program) = parts.next() else {
        return stdout::stream_stdout(content);
    };
    let args: Vec<&str> = parts.collect();

    let child = Command::new(program)
        .args(args)
        .stdin(Stdio::piped())
        .stderr(if suppress_stderr() {
            Stdio::null()
        } else {
            Stdio::inherit()
        })
        .spawn();
    let mut child = match child {
        Ok(child) => child,
        Err(_) => {
            return stdout::stream_stdout(content);
        }
    };

    let stdin = child.stdin.take();
    // Keep the pager in the foreground process group so interactive terminal
    // reads still work. Monitor it while either copy or wait can be blocked.
    std::thread::scope(|scope| {
        let monitor = scope.spawn(move || wait_pager(child));
        let written = match stdin {
            Some(mut stdin) => copy_output(content, &mut stdin),
            None => Ok(()),
        };
        monitor
            .join()
            .map_err(|_| GbError::Other("Pager monitor panicked".into()))??;
        Ok(written?)
    })
}

fn wait_pager(mut child: std::process::Child) -> Result<()> {
    let result = (|| loop {
        if crate::cancel::requested() {
            break Err(GbError::Cancelled);
        }
        if let Some(status) = child.try_wait()? {
            if status.success() {
                return Ok(());
            }
            return Err(GbError::Pager {
                status: status.code(),
            });
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    })();
    if result.is_err() {
        let _ = child.kill();
        let _ = child.wait();
    }
    result
}

fn copy_output(content: &mut impl Read, output: &mut impl Write) -> std::io::Result<()> {
    match std::io::copy(content, output) {
        Ok(_) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::BrokenPipe => Ok(()),
        Err(error) => Err(error),
    }
}

#[cfg(test)]
mod tests {
    use super::truncate;

    #[test]
    fn truncates_ascii_strings() {
        assert_eq!(truncate("abcdef", 5), "ab...");
        assert_eq!(truncate("abc", 5), "abc");
    }

    #[test]
    fn truncates_multibyte_strings_without_panicking() {
        assert_eq!(truncate("こんにちは", 4), "...");
        assert_eq!(truncate("こんにちは", 2), "こ");
    }
}
