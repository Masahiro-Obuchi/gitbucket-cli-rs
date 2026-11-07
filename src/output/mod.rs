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

pub fn stderr_line(args: std::fmt::Arguments<'_>) {
    if suppress_stderr() {
        return;
    }
    let mut stderr = std::io::stderr().lock();
    let _ = stderr.write_fmt(args);
    let _ = stderr.write_all(b"\n");
}

pub fn stderr_write(args: std::fmt::Arguments<'_>) {
    if suppress_stderr() {
        return;
    }
    let mut stderr = std::io::stderr().lock();
    let _ = stderr.write_fmt(args);
}

pub fn print_json<T: Serialize>(value: &T) -> Result<()> {
    println!("{}", serde_json::to_string_pretty(value)?);
    Ok(())
}

pub fn page_json<T: Serialize>(value: &T, no_pager: bool) -> Result<()> {
    page_or_print(
        &format!("{}\n", serde_json::to_string_pretty(value)?),
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
        "merged" => "MERGED".magenta().bold().to_string(),
        _ => state.to_string(),
    }
}

/// Truncate a string to a max width, appending "..." if needed
pub fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else if max > 3 {
        let prefix: String = s.chars().take(max - 3).collect();
        format!("{prefix}...")
    } else {
        s.chars().take(max).collect()
    }
}

pub fn page_or_print(content: &str, no_pager: bool) -> std::io::Result<()> {
    page_reader(&mut content.as_bytes(), no_pager)
}

/// Stream validated output, closing stdin and reaping even an early-exit pager.
pub fn page_reader(content: &mut impl Read, no_pager: bool) -> std::io::Result<()> {
    if no_pager || !std::io::stdout().is_terminal() {
        return copy_output(content, &mut std::io::stdout().lock());
    }

    let pager = std::env::var("PAGER").unwrap_or_else(|_| "less -R".to_string());
    let mut parts = pager.split_whitespace();
    let Some(program) = parts.next() else {
        return copy_output(content, &mut std::io::stdout().lock());
    };
    let args: Vec<&str> = parts.collect();

    let child = Command::new(program)
        .args(args)
        .stdin(Stdio::piped())
        .spawn();
    let mut child = match child {
        Ok(child) => child,
        Err(_) => {
            return copy_output(content, &mut std::io::stdout().lock());
        }
    };

    let result = match child.stdin.take() {
        Some(mut stdin) => copy_output(content, &mut stdin),
        None => Ok(()),
    };
    let waited = child.wait();
    result?;
    waited.map(|_| ())
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
        assert_eq!(truncate("こんにちは", 4), "こ...");
        assert_eq!(truncate("こんにちは", 2), "こん");
    }
}
