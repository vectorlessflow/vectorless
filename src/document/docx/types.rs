// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! DOCX-specific type definitions.

/// Parsed DOCX paragraph.
#[derive(Debug, Clone)]
pub struct DocxParagraph {
    /// Text content.
    pub text: String,
    /// Style ID (e.g., "Heading1", "Normal").
    pub style_id: Option<String>,
    /// Detected heading level (1-6), None for body text.
    pub heading_level: Option<u8>,
}

impl DocxParagraph {
    /// Create a new paragraph.
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            style_id: None,
            heading_level: None,
        }
    }

    /// Check if this paragraph has content.
    pub fn has_content(&self) -> bool {
        !self.text.trim().is_empty()
    }

    /// Check if this is a heading.
    pub fn is_heading(&self) -> bool {
        self.heading_level.is_some()
    }
}

/// Parsed style definition.
#[derive(Debug, Clone)]
pub struct DocxStyle {
    /// Style ID (e.g., "Heading1").
    pub style_id: String,
    /// Style name (e.g., "heading 1").
    pub name: Option<String>,
    /// Whether this style is a heading.
    pub is_heading: bool,
    /// Heading level (1-6), if this is a heading.
    pub heading_level: Option<u8>,
}

impl DocxStyle {
    /// Create a new style.
    pub fn new(style_id: impl Into<String>) -> Self {
        Self {
            style_id: style_id.into(),
            name: None,
            is_heading: false,
            heading_level: None,
        }
    }

    /// Create a heading style.
    pub fn heading(style_id: impl Into<String>, level: u8) -> Self {
        Self {
            style_id: style_id.into(),
            name: Some(format!("heading {}", level)),
            is_heading: true,
            heading_level: Some(level),
        }
    }
}
