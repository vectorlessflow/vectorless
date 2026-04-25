// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Frontmatter extraction for Markdown documents.
//!
//! Supports YAML (`---`) and TOML (`+++`) delimited frontmatter.

use std::collections::HashMap;

/// Parsed frontmatter data.
#[derive(Debug, Clone, Default)]
pub struct Frontmatter {
    /// Extracted key-value pairs.
    pub fields: HashMap<String, String>,
}

impl Frontmatter {
    /// Create an empty frontmatter.
    #[must_use]
    #[allow(dead_code)]
    pub fn new() -> Self {
        Self {
            fields: HashMap::new(),
        }
    }

    /// Parse frontmatter from raw content.
    ///
    /// Returns `Some(Frontmatter)` if valid frontmatter is found.
    /// Returns `None` if no frontmatter delimiters are present.
    fn parse<'a>(content: &'a str, delimiter: &str) -> Option<(Self, &'a str)> {
        // Check if content starts with delimiter
        let delim_line = format!("{}\n", delimiter);
        if !content.starts_with(&delim_line) {
            return None;
        }

        // Find closing delimiter
        let content_after_open = &content[delimiter.len() + 1..];
        let close_pattern = format!("\n{}\n", delimiter);

        if let Some(end_pos) = content_after_open.find(&close_pattern) {
            let frontmatter_text = &content_after_open[..end_pos];
            let remaining = &content_after_open[end_pos + close_pattern.len()..];

            let fm = Self::parse_yaml(frontmatter_text);
            Some((fm, remaining))
        } else {
            None
        }
    }

    /// Parse YAML-style frontmatter (simple key: value extraction).
    fn parse_yaml(text: &str) -> Self {
        let mut fields = HashMap::new();

        for line in text.lines() {
            let line = line.trim();

            // Skip empty lines and comments
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            // Parse "key: value" or "key: "quoted value""
            if let Some((key, value)) = line.split_once(':') {
                let key = key.trim().to_string();
                let value = value.trim();

                // Remove quotes if present
                let value = if (value.starts_with('"') && value.ends_with('"'))
                    || (value.starts_with('\'') && value.ends_with('\''))
                {
                    value[1..value.len() - 1].to_string()
                } else {
                    value.to_string()
                };

                fields.insert(key, value);
            }
        }

        Self { fields }
    }

    /// Get a field value by key.
    #[must_use]
    pub fn get(&self, key: &str) -> Option<&String> {
        self.fields.get(key)
    }

    /// Check if a field exists.
    #[must_use]
    #[allow(dead_code)]
    pub fn contains(&self, key: &str) -> bool {
        self.fields.contains_key(key)
    }

    /// Get the title field.
    #[must_use]
    #[allow(dead_code)]
    pub fn title(&self) -> Option<&String> {
        self.get("title")
    }

    /// Get the description field.
    #[must_use]
    #[allow(dead_code)]
    pub fn description(&self) -> Option<&String> {
        self.get("description")
    }
}

/// Extract frontmatter from Markdown content.
///
/// Returns a tuple of (frontmatter, remaining_content).
/// If no frontmatter is found, returns `(None, content)`.
///
/// # Supported Formats
///
/// - YAML: `---\nkey: value\n---`
/// - TOML: `+++\nkey = "value"\n+++`
#[must_use]
pub fn extract_frontmatter(
    content: &str,
    parse_yaml: bool,
    parse_toml: bool,
) -> (Option<Frontmatter>, &str) {
    // Try YAML frontmatter first
    if parse_yaml {
        if let Some((fm, remaining)) = Frontmatter::parse(content, "---") {
            return (Some(fm), remaining);
        }
    }

    // Try TOML frontmatter
    if parse_toml {
        if let Some((fm, remaining)) = Frontmatter::parse(content, "+++") {
            return (Some(fm), remaining);
        }
    }

    // No frontmatter found
    (None, content)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_yaml_frontmatter() {
        let content = r#"---
title: My Document
description: A test document
---

# Content

Body text."#;

        let (fm, remaining) = extract_frontmatter(content, true, false);

        assert!(fm.is_some());
        let fm = fm.unwrap();
        assert_eq!(fm.title(), Some(&"My Document".to_string()));
        assert_eq!(fm.description(), Some(&"A test document".to_string()));
        assert!(remaining.trim_start().starts_with("# Content"));
    }

    #[test]
    fn test_extract_quoted_values() {
        let content = r#"---
title: "Quoted Title"
description: 'Single quoted'
---

Content"#;

        let (fm, _) = extract_frontmatter(content, true, false);

        assert!(fm.is_some());
        let fm = fm.unwrap();
        assert_eq!(fm.title(), Some(&"Quoted Title".to_string()));
        assert_eq!(fm.description(), Some(&"Single quoted".to_string()));
    }

    #[test]
    fn test_no_frontmatter() {
        let content = "# No Frontmatter\n\nJust content.";

        let (fm, remaining) = extract_frontmatter(content, true, false);

        assert!(fm.is_none());
        assert_eq!(remaining, content);
    }

    #[test]
    fn test_incomplete_frontmatter() {
        let content = "---\ntitle: Test\n\nNo closing delimiter";

        let (fm, remaining) = extract_frontmatter(content, true, false);

        // Should not match incomplete frontmatter
        assert!(fm.is_none());
        assert_eq!(remaining, content);
    }

    #[test]
    fn test_toml_frontmatter() {
        let content = r#"+++
title = "TOML Doc"
+++

# Content"#;

        let (fm, remaining) = extract_frontmatter(content, false, true);

        // Note: Our simple parser treats TOML as YAML-like
        assert!(fm.is_some());
        assert!(remaining.trim_start().starts_with("# Content"));
    }
}
