pub mod table;

use colored::Colorize;

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
