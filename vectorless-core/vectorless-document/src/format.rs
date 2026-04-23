// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Document format and sufficiency types.
//!
//! These types are used across multiple modules and are defined here
//! to avoid circular dependencies between crates.

use serde::{Deserialize, Serialize};

/// Supported document formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DocumentFormat {
    /// Markdown files (.md, .markdown)
    Markdown,
    /// PDF files (.pdf)
    Pdf,
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
    pub fn extension(&self) -> &'static str {
        match self {
            Self::Markdown => "md",
            Self::Pdf => "pdf",
        }
    }

    /// All supported file extensions (lowercase).
    ///
    /// Single source of truth — used by directory scanning to
    /// discover indexable files.
    pub const SUPPORTED_EXTENSIONS: &'static [&'static str] = &["md", "pdf"];
}

/// Sufficiency level for incremental retrieval.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SufficiencyLevel {
    /// Information is sufficient, stop retrieving.
    Sufficient,

    /// Partial information, can continue if needed.
    PartialSufficient,

    /// Information is insufficient, continue retrieving.
    Insufficient,
}

impl Default for SufficiencyLevel {
    fn default() -> Self {
        Self::Insufficient
    }
}
