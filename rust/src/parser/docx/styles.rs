// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Style resolution for DOCX documents.
//!
//! This module handles the detection of heading styles from DOCX documents.

use std::collections::HashMap;

use super::types::DocxStyle;

/// Word namespace URI.
const WORD_NS: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";

/// Style resolver for mapping style IDs to heading levels.
#[derive(Debug, Clone, Default)]
pub struct StyleResolver {
    /// Map from style_id to resolved style info.
    styles: HashMap<String, DocxStyle>,
}

impl StyleResolver {
    /// Create a new empty style resolver.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a style resolver from styles.xml content.
    pub fn from_xml(styles_xml: &str) -> Self {
        let mut resolver = Self::new();

        // Add built-in styles first
        resolver.add_builtin_styles();

        // Parse styles.xml if available
        if !styles_xml.is_empty() {
            resolver.parse_styles_xml(styles_xml);
        }

        resolver
    }

    /// Add built-in heading styles.
    fn add_builtin_styles(&mut self) {
        // Standard Word heading styles
        for level in 1..=6 {
            let style_id = format!("Heading{}", level);
            self.styles
                .insert(style_id.clone(), DocxStyle::heading(&style_id, level));
        }

        // Some documents use lowercase or different casing
        for level in 1..=6 {
            let style_id = format!("heading{}", level);
            self.styles
                .insert(style_id.clone(), DocxStyle::heading(&style_id, level));
        }

        // Title style (treat as H1)
        self.styles
            .insert("Title".to_string(), DocxStyle::heading("Title", 1));
    }

    /// Parse styles.xml content.
    fn parse_styles_xml(&mut self, styles_xml: &str) {
        let doc = match roxmltree::Document::parse(styles_xml) {
            Ok(doc) => doc,
            Err(_) => return,
        };

        // Find all w:style elements
        for style_elem in doc
            .descendants()
            .filter(|n| n.has_tag_name((WORD_NS, "style")))
        {
            if let Some(style) = self.parse_style_element(&style_elem) {
                self.styles.insert(style.style_id.clone(), style);
            }
        }
    }

    /// Parse a single w:style element.
    fn parse_style_element(&self, elem: &roxmltree::Node) -> Option<DocxStyle> {
        // Get style ID
        let style_id = elem.attribute((WORD_NS, "styleId"))?.to_string();

        let mut style = DocxStyle::new(&style_id);

        // Get style name
        for child in elem.children() {
            if child.has_tag_name((WORD_NS, "name")) {
                if let Some(name) = child.attribute((WORD_NS, "val")) {
                    style.name = Some(name.to_string());

                    // Check if name indicates a heading
                    let name_lower = name.to_lowercase();
                    if name_lower.starts_with("heading") {
                        style.is_heading = true;
                        // Extract heading level from name
                        if let Some(level) = self.extract_heading_level(&name_lower) {
                            style.heading_level = Some(level);
                        }
                    }
                }
            }

            // Check for outline level (indicates heading)
            if child.has_tag_name((WORD_NS, "pPr")) {
                for ppr_child in child.children() {
                    if ppr_child.has_tag_name((WORD_NS, "outlineLvl")) {
                        if let Some(level_str) = ppr_child.attribute((WORD_NS, "val")) {
                            if let Ok(level) = level_str.parse::<u8>() {
                                style.is_heading = true;
                                // outlineLvl is 0-indexed, heading level is 1-indexed
                                style.heading_level = Some(level + 1);
                            }
                        }
                    }
                }
            }
        }

        Some(style)
    }

    /// Extract heading level from a style name.
    fn extract_heading_level(&self, name: &str) -> Option<u8> {
        // Try to extract number from "heading N" or "headingN"
        let digits: String = name.chars().filter(|c| c.is_ascii_digit()).collect();
        digits.parse().ok().filter(|&l| l >= 1 && l <= 6)
    }

    /// Get heading level for a style ID.
    pub fn get_heading_level(&self, style_id: &Option<String>) -> Option<u8> {
        style_id
            .as_ref()
            .and_then(|id| self.styles.get(id).and_then(|s| s.heading_level))
    }

    /// Check if a style is a heading.
    pub fn is_heading(&self, style_id: &Option<String>) -> bool {
        style_id
            .as_ref()
            .is_some_and(|id| self.styles.get(id).is_some_and(|s| s.is_heading))
    }

    /// Try to detect heading level from text content heuristics.
    ///
    /// This is used when no style information is available.
    pub fn detect_heading_by_heuristics(&self, text: &str) -> Option<u8> {
        let text = text.trim();

        // Skip very long texts (unlikely to be headings)
        if text.len() > 100 {
            return None;
        }

        // Check for common heading patterns
        // Pattern: "Chapter X" or "Section X"
        let text_lower = text.to_lowercase();
        if text_lower.starts_with("chapter ") || text_lower.starts_with("section ") {
            return Some(1);
        }

        // Pattern: numbered sections like "1.", "1.1", "1.1.1"
        let numbered_level = self.detect_numbered_heading(text);
        if numbered_level.is_some() {
            return numbered_level;
        }

        None
    }

    /// Detect heading level from numbered patterns.
    fn detect_numbered_heading(&self, text: &str) -> Option<u8> {
        // Match patterns like "1.", "1.1", "1.1.1", etc.
        let mut depth = 0u8;
        let mut prev_was_digit = false;
        let mut has_digit = false;

        for ch in text.chars() {
            if ch.is_ascii_digit() {
                prev_was_digit = true;
                has_digit = true;
            } else if ch == '.' && prev_was_digit {
                depth += 1;
                prev_was_digit = false;
            } else if ch == ' ' && prev_was_digit {
                // End of number sequence
                depth += 1;
                break;
            } else if !ch.is_whitespace() && has_digit {
                // Non-digit, non-dot, non-space after digits
                break;
            }
        }

        if depth > 0 && depth <= 6 {
            Some(depth)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builtin_styles() {
        let resolver = StyleResolver::new();
        let resolver = {
            let mut r = resolver;
            r.add_builtin_styles();
            r
        };

        assert_eq!(
            resolver.get_heading_level(&Some("Heading1".to_string())),
            Some(1)
        );
        assert_eq!(
            resolver.get_heading_level(&Some("Heading2".to_string())),
            Some(2)
        );
        assert_eq!(
            resolver.get_heading_level(&Some("Normal".to_string())),
            None
        );
    }

    #[test]
    fn test_detect_numbered_heading() {
        let resolver = StyleResolver::new();

        assert_eq!(resolver.detect_numbered_heading("1. Introduction"), Some(1));
        assert_eq!(resolver.detect_numbered_heading("1.1 Background"), Some(2));
        assert_eq!(resolver.detect_numbered_heading("1.1.1 Details"), Some(3));
        assert_eq!(resolver.detect_numbered_heading("Introduction"), None);
    }

    #[test]
    fn test_detect_heading_by_heuristics() {
        let resolver = StyleResolver::new();

        assert_eq!(resolver.detect_heading_by_heuristics("Chapter 1"), Some(1));
        assert_eq!(resolver.detect_heading_by_heuristics("Section 2"), Some(1));
        assert_eq!(
            resolver.detect_heading_by_heuristics("1. Introduction"),
            Some(1)
        );
        assert_eq!(
            resolver.detect_heading_by_heuristics("1.1 Background"),
            Some(2)
        );
        assert_eq!(
            resolver.detect_heading_by_heuristics(
                "This is a very long piece of text that is unlikely to be a heading"
            ),
            None
        );
    }
}
