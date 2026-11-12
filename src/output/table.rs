use super::text::{cell, width};
use colored::Colorize;

/// Print a simple table with aligned columns
pub fn print_table(headers: &[&str], rows: &[Vec<String>]) {
    print!("{}", format_table(headers, rows));
}

/// Format a simple table with aligned columns
pub fn format_table(headers: &[&str], rows: &[Vec<String>]) -> String {
    if rows.is_empty() {
        return "No items found.\n".to_string();
    }

    let headers: Vec<_> = headers.iter().map(|h| cell(h)).collect();
    let rows: Vec<Vec<_>> = rows
        .iter()
        .map(|row| row.iter().map(|value| cell(value)).collect())
        .collect();
    // Calculate column widths
    let mut widths: Vec<usize> = headers.iter().map(|h| width(h)).collect();
    for row in &rows {
        for (i, cell) in row.iter().enumerate() {
            if i < widths.len() {
                widths[i] = widths[i].max(width(cell));
            }
        }
    }

    // Print header
    let header_line: Vec<String> = headers
        .iter()
        .enumerate()
        .map(|(i, h)| format!("{}{}", h, " ".repeat(widths[i] - width(h))))
        .collect();
    let mut output = String::new();
    output.push_str(&header_line.join("  ").dimmed().to_string());
    output.push('\n');

    // Print rows
    for row in &rows {
        let line: Vec<String> = row
            .iter()
            .enumerate()
            .map(|(i, cell)| {
                let plain_len = width(cell);
                let column_width = widths.get(i).copied().unwrap_or(0);
                let padding = if column_width > plain_len {
                    " ".repeat(column_width - plain_len)
                } else {
                    String::new()
                };
                format!("{}{}", cell, padding)
            })
            .collect();
        output.push_str(&line.join("  "));
        output.push('\n');
    }

    output
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unicode_cells_align_and_do_not_inject_rows_or_terminal_commands() {
        let result = format_table(
            &["名前", "N"],
            &[
                vec!["e\u{301}".into(), "1".into()],
                vec!["👩‍💻".into(), "2".into()],
            ],
        );
        let lines: Vec<_> = result.lines().collect();
        assert_eq!(lines[1], "e\u{301}     1");
        assert_eq!(lines[2], "👩‍💻    2");
        let result = format_table(&["NAME"], &[vec!["x\n\x1b[2J\ry".into()]]);
        assert_eq!(result.lines().count(), 2);
        assert!(!result.contains('\x1b'));
        assert!(result.contains("x\\n\\u{1b}[2J\\ry"));
    }
}
