use colored::Colorize;

/// Print a simple table with aligned columns
pub fn print_table(headers: &[&str], rows: &[Vec<String>]) {
    if rows.is_empty() {
        println!("No items found.");
        return;
    }

    // Calculate column widths
    let mut widths: Vec<usize> = headers.iter().map(|h| h.chars().count()).collect();
    for row in rows {
        for (i, cell) in row.iter().enumerate() {
            if i < widths.len() {
                // Strip ANSI codes for width calculation
                let plain = strip_ansi(cell);
                widths[i] = widths[i].max(plain.chars().count());
            }
        }
    }

    // Print header
    let header_line: Vec<String> = headers
        .iter()
        .enumerate()
        .map(|(i, h)| format!("{:<width$}", h, width = widths[i]))
        .collect();
    println!("{}", header_line.join("  ").dimmed());

    // Print rows
    for row in rows {
        let line: Vec<String> = row
            .iter()
            .enumerate()
            .map(|(i, cell)| {
                let plain_len = strip_ansi(cell).chars().count();
                let width = widths.get(i).copied().unwrap_or(0);
                let padding = if width > plain_len {
                    " ".repeat(width - plain_len)
                } else {
                    String::new()
                };
                format!("{}{}", cell, padding)
            })
            .collect();
        println!("{}", line.join("  "));
    }
}

fn strip_ansi(s: &str) -> String {
    let mut result = String::new();
    let mut in_escape = false;
    for c in s.chars() {
        if c == '\x1b' {
            in_escape = true;
        } else if in_escape {
            if c == 'm' {
                in_escape = false;
            }
        } else {
            result.push(c);
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::strip_ansi;

    #[test]
    fn strip_ansi_removes_color_codes() {
        let colored = "\u{1b}[31mERROR\u{1b}[0m";
        assert_eq!(strip_ansi(colored), "ERROR");
    }

    #[test]
    fn strip_ansi_keeps_plain_text() {
        assert_eq!(strip_ansi("plain text"), "plain text");
    }
}
