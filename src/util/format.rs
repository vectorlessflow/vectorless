// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Text formatting utilities.

/// Truncate text to a maximum length with ellipsis.
///
/// # Example
///
/// ```
/// use vectorless::util::truncate;
///
/// assert_eq!(truncate("hello world", 8), "hello...");
/// assert_eq!(truncate("hi", 10), "hi");
/// ```
pub fn truncate(text: &str, max_len: usize) -> String {
    if text.len() <= max_len {
        return text.to_string();
    }

    if max_len <= 3 {
        return ".".repeat(max_len);
    }

    format!("{}...", &text[..max_len - 3])
}

/// Truncate text to a maximum length, respecting word boundaries.
pub fn truncate_words(text: &str, max_len: usize) -> String {
    if text.len() <= max_len {
        return text.to_string();
    }

    if max_len <= 3 {
        return ".".repeat(max_len);
    }

    // Find a good break point
    let truncated = &text[..max_len - 3];

    // Try to break at a word boundary
    if let Some(last_space) = truncated.rfind(' ') {
        if last_space > max_len / 2 {
            return format!("{}...", &truncated[..last_space]);
        }
    }

    format!("{}...", truncated)
}

/// Format a number with thousand separators.
///
/// # Example
///
/// ```
/// use vectorless::util::format_number;
///
/// assert_eq!(format_number(1000), "1,000");
/// assert_eq!(format_number(1234567), "1,234,567");
/// ```
pub fn format_number(n: usize) -> String {
    let s = n.to_string();
    let mut result = String::new();
    let chars: Vec<char> = s.chars().collect();

    for (i, c) in chars.iter().enumerate() {
        if i > 0 && (chars.len() - i) % 3 == 0 {
            result.push(',');
        }
        result.push(*c);
    }

    result
}

/// Format bytes for human-readable display.
///
/// # Example
///
/// ```
/// use vectorless::util::format_bytes;
///
/// assert_eq!(format_bytes(500), "500 B");
/// assert_eq!(format_bytes(1024), "1.0 KB");
/// assert_eq!(format_bytes(1536), "1.5 KB");
/// assert_eq!(format_bytes(1048576), "1.0 MB");
/// ```
pub fn format_bytes(bytes: usize) -> String {
    const KB: usize = 1024;
    const MB: usize = KB * 1024;
    const GB: usize = MB * 1024;

    if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

/// Format a percentage.
///
/// # Example
///
/// ```
/// use vectorless::util::format_percent;
///
/// assert_eq!(format_percent(0.5), "50.0%");
/// assert_eq!(format_percent(0.123), "12.3%");
/// ```
pub fn format_percent(value: f32) -> String {
    format!("{:.1}%", value * 100.0)
}

/// Clean whitespace in text (collapse multiple spaces, trim).
pub fn clean_whitespace(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Indent each line of text.
pub fn indent(text: &str, spaces: usize) -> String {
    let indent_str = " ".repeat(spaces);
    text.lines()
        .map(|line| format!("{}{}", indent_str, line))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Count words in text.
pub fn word_count(text: &str) -> usize {
    text.split_whitespace().count()
}

/// Count lines in text.
pub fn line_count(text: &str) -> usize {
    if text.is_empty() {
        return 0;
    }
    text.chars().filter(|&c| c == '\n').count() + 1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_truncate() {
        assert_eq!(truncate("hello", 10), "hello");
        assert_eq!(truncate("hello world", 8), "hello...");
        assert_eq!(truncate("hi", 3), "hi");
    }

    #[test]
    fn test_truncate_words() {
        // "hello world foo" with max_len=12:
        // truncated = "hello wor" (9 chars), last_space at 5
        // 5 > 12/2 is false, so no word boundary break
        assert_eq!(truncate_words("hello world foo", 12), "hello wor...");
        // Word boundary break happens when space is past halfway
        assert_eq!(truncate_words("hello world foo bar", 15), "hello world...");
        assert_eq!(truncate_words("hello", 10), "hello");
    }

    #[test]
    fn test_format_number() {
        assert_eq!(format_number(100), "100");
        assert_eq!(format_number(1000), "1,000");
        assert_eq!(format_number(1234567), "1,234,567");
    }

    #[test]
    fn test_format_bytes() {
        assert_eq!(format_bytes(500), "500 B");
        assert_eq!(format_bytes(1024), "1.0 KB");
        assert_eq!(format_bytes(1536), "1.5 KB");
        assert_eq!(format_bytes(1048576), "1.0 MB");
    }

    #[test]
    fn test_format_percent() {
        assert_eq!(format_percent(0.5), "50.0%");
        assert_eq!(format_percent(1.0), "100.0%");
    }

    #[test]
    fn test_clean_whitespace() {
        assert_eq!(clean_whitespace("  hello   world  "), "hello world");
        assert_eq!(clean_whitespace("single"), "single");
    }

    #[test]
    fn test_indent() {
        assert_eq!(indent("hello\nworld", 2), "  hello\n  world");
    }

    #[test]
    fn test_word_count() {
        assert_eq!(word_count("hello world"), 2);
        assert_eq!(word_count("  hello   world  "), 2);
        assert_eq!(word_count(""), 0);
    }

    #[test]
    fn test_line_count() {
        assert_eq!(line_count("hello\nworld"), 2);
        assert_eq!(line_count("single"), 1);
        assert_eq!(line_count(""), 0);
    }
}
