// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Text analysis utilities for query understanding.
//!
//! Migrated from `agent::subagent` private functions so they can be shared
//! across modules.

/// Estimate word count, handling both CJK and Latin text.
///
/// Each CJK character counts as one word. Latin words are split on whitespace.
pub fn estimate_word_count(text: &str) -> usize {
    let mut count = 0usize;
    let mut in_latin_word = false;
    for ch in text.chars() {
        if ch.is_whitespace() {
            if in_latin_word {
                count += 1;
                in_latin_word = false;
            }
        } else if ch.is_ascii_alphanumeric() {
            in_latin_word = true;
        } else if is_cjk_char(ch) {
            if in_latin_word {
                count += 1;
                in_latin_word = false;
            }
            count += 1;
        } else if in_latin_word {
            count += 1;
            in_latin_word = false;
        }
    }
    if in_latin_word {
        count += 1;
    }
    count
}

/// Check if a character is CJK (Chinese/Japanese/Korean).
pub fn is_cjk_char(ch: char) -> bool {
    let cp = ch as u32;
    (0x4E00..=0x9FFF).contains(&cp)
        || (0x3400..=0x4DBF).contains(&cp)
        || (0x20000..=0x2A6DF).contains(&cp)
        || (0xF900..=0xFAFF).contains(&cp)
        || (0x3000..=0x303F).contains(&cp)
        || (0x3040..=0x309F).contains(&cp)
        || (0x30A0..=0x30FF).contains(&cp)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn latin_words() {
        assert_eq!(estimate_word_count("hello world"), 2);
        assert_eq!(estimate_word_count("one two three four"), 4);
    }

    #[test]
    fn cjk_chars() {
        // Each CJK char is one word
        assert_eq!(estimate_word_count("\u{4f60}\u{597d}\u{4e16}\u{754c}"), 4);
    }

    #[test]
    fn mixed() {
        // "hello" (1 latin word) + space + 2 CJK chars = 3 words total
        assert_eq!(estimate_word_count("hello \u{4e16}\u{754c}"), 3);
    }

    #[test]
    fn empty() {
        assert_eq!(estimate_word_count(""), 0);
    }

    #[test]
    fn cjk_detection() {
        assert!(is_cjk_char('\u{4e2d}'));
        assert!(is_cjk_char('\u{3042}')); // Hiragana range (0x3040-0x309F)
        assert!(!is_cjk_char('a'));
        assert!(!is_cjk_char(' '));
    }
}
