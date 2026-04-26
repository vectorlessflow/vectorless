// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Document format types.

use serde::{Deserialize, Serialize};

/// Supported document formats.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum DocumentFormat {
    /// Markdown files (.md, .markdown)
    Markdown,
    /// PDF files (.pdf)
    Pdf,
    /// Custom format identified by name (for parser plugins).
    Custom(String),
}

impl Serialize for DocumentFormat {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Markdown => serializer.serialize_str("markdown"),
            Self::Pdf => serializer.serialize_str("pdf"),
            Self::Custom(name) => serializer.serialize_str(name),
        }
    }
}

impl<'de> Deserialize<'de> for DocumentFormat {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        match s.as_str() {
            "markdown" => Ok(Self::Markdown),
            "pdf" => Ok(Self::Pdf),
            _ => Ok(Self::Custom(s)),
        }
    }
}

impl DocumentFormat {
    /// Detect format from file extension.
    pub fn from_extension(ext: &str) -> Option<Self> {
        match ext.to_lowercase().as_str() {
            "md" | "markdown" => Some(Self::Markdown),
            "pdf" => Some(Self::Pdf),
            _ => None,
        }
    }

    /// Get the file extension for this format.
    pub fn extension(&self) -> &str {
        match self {
            Self::Markdown => "md",
            Self::Pdf => "pdf",
            Self::Custom(name) => name,
        }
    }

    /// All supported file extensions (lowercase).
    ///
    /// Single source of truth — used by directory scanning to
    /// discover indexable files.
    pub const SUPPORTED_EXTENSIONS: &'static [&'static str] = &["md", "pdf"];
}
