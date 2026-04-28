pub mod table;

use colored::Colorize;
use std::io::{IsTerminal, Write};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};

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
    if no_pager || !std::io::stdout().is_terminal() {
        print!("{content}");
        return Ok(());
    }

    let pager = std::env::var("PAGER").unwrap_or_else(|_| "less -R".to_string());
    let mut parts = pager.split_whitespace();
    let Some(program) = parts.next() else {
        print!("{content}");
        return Ok(());
    };
    let args: Vec<&str> = parts.collect();

    let child = Command::new(program)
        .args(args)
        .stdin(Stdio::piped())
        .spawn();
    let mut child = match child {
        Ok(child) => child,
        Err(_) => {
            print!("{content}");
            return Ok(());
        }
    };

    if let Some(stdin) = child.stdin.as_mut() {
        if let Err(err) = stdin.write_all(content.as_bytes()) {
            if err.kind() == std::io::ErrorKind::BrokenPipe {
                return Ok(());
            }
            return Err(err);
        }
    }
    let _ = child.stdin.take();
    let _ = child.wait();
    Ok(())
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
