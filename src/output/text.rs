//! Human display only. JSON, completion and raw diff streams bypass this layer.
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

// Only basic colors, bold/dim and their resets are terminal decorations.
// Cursor control, hyperlinks, conceal/blink and arbitrary CSI/OSC are data.
fn sgr(s: &str) -> Option<usize> {
    let rest = s.strip_prefix("\x1b[")?;
    let end = rest.find(|c: char| !c.is_ascii_digit() && c != ';')?;
    if rest.as_bytes()[end] != b'm' {
        return None;
    }
    let valid = rest[..end].split(';').all(|code| {
        code.is_empty()
            || code.parse::<u16>().is_ok_and(
                |n| matches!(n, 0..=2 | 22 | 30..=37 | 39 | 40..=47 | 49 | 90..=97 | 100..=107),
            )
    });
    valid.then_some(2 + end + 1)
}

fn sanitize(s: &str, multiline: bool) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while !rest.is_empty() {
        if let Some(len) = sgr(rest) {
            out.push_str(&rest[..len]);
            rest = &rest[len..];
            continue;
        }
        let c = rest.chars().next().unwrap();
        rest = &rest[c.len_utf8()..];
        if (c == '\n' || c == '\t') && multiline {
            out.push(c);
        } else if c.is_control()
            || matches!(c, '\u{061c}' | '\u{200e}' | '\u{200f}' | '\u{2028}'..='\u{202e}' | '\u{2066}'..='\u{2069}')
        {
            out.extend(c.escape_default());
        } else {
            out.push(c);
        }
    }
    out
}

pub(super) fn human(s: &str) -> String {
    sanitize(s, true)
}
pub(super) fn cell(s: &str) -> String {
    sanitize(s, false)
}

fn plain(s: &str) -> String {
    let mut out = String::new();
    let mut rest = s;
    while !rest.is_empty() {
        if let Some(len) = sgr(rest) {
            rest = &rest[len..];
        } else {
            let c = rest.chars().next().unwrap();
            out.push(c);
            rest = &rest[c.len_utf8()..];
        }
    }
    out
}

pub(super) fn width(s: &str) -> usize {
    UnicodeWidthStr::width(plain(s).as_str())
}

pub(super) fn truncate(s: &str, max: usize) -> String {
    let text = plain(&cell(s));
    if UnicodeWidthStr::width(text.as_str()) <= max {
        return text;
    }
    let suffix = if max > 3 { "..." } else { "" };
    let available = max - suffix.len();
    let mut out = String::new();
    for grapheme in text.graphemes(true) {
        if UnicodeWidthStr::width(out.as_str()) + UnicodeWidthStr::width(grapheme) > available {
            break;
        }
        out.push_str(grapheme);
    }
    out.push_str(suffix);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn escapes_terminal_commands_but_keeps_limited_decoration_and_body_lines() {
        assert_eq!(
            human("a\r\x1b[2J\x1b]8;;url\x07\u{009b}1J\u{202e}\n\tend"),
            "a\\r\\u{1b}[2J\\u{1b}]8;;url\\u{7}\\u{9b}1J\\u{202e}\n\tend"
        );
        assert_eq!(human("\x1b[1;32mok\x1b[0m"), "\x1b[1;32mok\x1b[0m");
        assert!(!human("\x1b[8mhidden\x1b[?25l").contains('\x1b'));
        assert_eq!(cell("a\n\tb\rc"), "a\\n\\tb\\rc");
    }
    #[test]
    fn unicode_width_and_truncation_keep_graphemes_whole() {
        assert_eq!(width("日本e\u{301}👩‍💻"), 7);
        assert_eq!(truncate("日本語", 5), "日...");
        assert_eq!(truncate("e\u{301}xy", 1), "e\u{301}");
        assert_eq!(truncate("👩‍💻ab", 2), "👩‍💻");
        assert_eq!(truncate("🇯🇵ab", 2), "🇯🇵");
        assert_eq!(truncate("👍🏽abc", 2), "👍🏽");
    }
}
