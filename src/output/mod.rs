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
    if s.len() <= max {
        s.to_string()
    } else if max > 3 {
        format!("{}...", &s[..max - 3])
    } else {
        s[..max].to_string()
    }
}
