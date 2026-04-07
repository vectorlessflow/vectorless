// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Configuration for HTML parsing.

use serde::{Deserialize, Serialize};

/// Configuration for HTML parsing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HtmlConfig {
    /// Default title for nodes without headings.
    #[serde(default = "default_title")]
    pub default_title: String,

    /// Minimum content length to keep a node.
    #[serde(default = "default_min_content_length")]
    pub min_content_length: usize,

    /// Whether to include code blocks.
    #[serde(default = "default_include_code_blocks")]
    pub include_code_blocks: bool,

    /// Whether to merge small consecutive nodes.
    #[serde(default = "default_merge_small_nodes")]
    pub merge_small_nodes: bool,

    /// Maximum heading level to process (1-6).
    #[serde(default = "default_max_heading_level")]
    pub max_heading_level: usize,
}

fn default_title() -> String {
    "Introduction".to_string()
}

fn default_min_content_length() -> usize {
    50
}

fn default_include_code_blocks() -> bool {
    true
}

fn default_merge_small_nodes() -> bool {
    true
}

fn default_max_heading_level() -> usize {
    6
}

impl Default for HtmlConfig {
    fn default() -> Self {
        Self {
            default_title: default_title(),
            min_content_length: default_min_content_length(),
            include_code_blocks: default_include_code_blocks(),
            merge_small_nodes: default_merge_small_nodes(),
            max_heading_level: default_max_heading_level(),
        }
    }
}

impl HtmlConfig {
    /// Create a new config with default values.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the default title for nodes without headings.
    #[must_use]
    pub fn with_default_title(mut self, title: impl Into<String>) -> Self {
        self.default_title = title.into();
        self
    }

    /// Set minimum content length to keep a node.
    #[must_use]
    pub fn with_min_content_length(mut self, len: usize) -> Self {
        self.min_content_length = len;
        self
    }

    /// Enable or disable code blocks.
    #[must_use]
    pub fn with_code_blocks(mut self, include: bool) -> Self {
        self.include_code_blocks = include;
        self
    }

    /// Enable or disable merging of small consecutive nodes.
    #[must_use]
    pub fn with_merge_small_nodes(mut self, merge: bool) -> Self {
        self.merge_small_nodes = merge;
        self
    }

    /// Set maximum heading level to process (1-6).
    #[must_use]
    pub fn with_max_heading_level(mut self, level: usize) -> Self {
        self.max_heading_level = level.clamp(1, 6);
        self
    }

    /// Create a config that excludes code blocks.
    #[must_use]
    pub fn no_code_blocks() -> Self {
        Self::new().with_code_blocks(false)
    }

    /// Create a config for simple documents (no merging).
    #[must_use]
    pub fn simple() -> Self {
        Self::new()
            .with_merge_small_nodes(false)
            .with_min_content_length(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = HtmlConfig::default();
        assert_eq!(config.default_title, "Introduction");
        assert_eq!(config.min_content_length, 50);
        assert!(config.include_code_blocks);
        assert!(config.merge_small_nodes);
        assert_eq!(config.max_heading_level, 6);
    }

    #[test]
    fn test_builder_pattern() {
        let config = HtmlConfig::new()
            .with_default_title("Overview")
            .with_min_content_length(100)
            .with_code_blocks(false)
            .with_max_heading_level(3);

        assert_eq!(config.default_title, "Overview");
        assert_eq!(config.min_content_length, 100);
        assert!(!config.include_code_blocks);
        assert_eq!(config.max_heading_level, 3);
    }

    #[test]
    fn test_max_heading_level_clamp() {
        let config = HtmlConfig::new().with_max_heading_level(10);
        assert_eq!(config.max_heading_level, 6);

        let config = HtmlConfig::new().with_max_heading_level(0);
        assert_eq!(config.max_heading_level, 1);
    }

    #[test]
    fn test_preset_configs() {
        let config = HtmlConfig::no_code_blocks();
        assert!(!config.include_code_blocks);

        let config = HtmlConfig::simple();
        assert!(!config.merge_small_nodes);
        assert_eq!(config.min_content_length, 0);
    }
}
