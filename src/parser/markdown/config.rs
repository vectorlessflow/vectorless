// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Configuration options for the Markdown parser.

/// Markdown parser configuration.
///
/// Controls parsing behavior, content extraction, and extension support.
///
/// # Example
///
/// ```rust
/// use vectorless::document::markdown::MarkdownConfig;
///
/// // Default GFM configuration
/// let config = MarkdownConfig::default();
///
/// // Strict CommonMark
/// let config = MarkdownConfig::commonmark();
///
/// // Documentation-focused
/// let config = MarkdownConfig::documentation();
///
/// // Custom configuration
/// let config = MarkdownConfig {
///     max_heading_level: 3,
///     include_code_blocks: false,
///     ..Default::default()
/// };
/// ```
#[derive(Debug, Clone)]
pub struct MarkdownConfig {
    // ============================================================
    // Parsing Options
    // ============================================================

    /// Enable GitHub Flavored Markdown extensions.
    ///
    /// Includes: tables, strikethrough, task lists, autolinks.
    /// Default: `true`
    pub enable_gfm: bool,

    /// Enable footnotes extension (`[^1]` syntax).
    /// Default: `false`
    pub enable_footnotes: bool,

    /// Enable definition lists.
    /// Default: `false`
    pub enable_definition_lists: bool,

    /// Enable superscript/subscript (`^sup^`, `~sub~`).
    /// Default: `false`
    pub enable_super_sub: bool,

    /// Maximum heading level to parse (1-6).
    /// Headings above this level are treated as content.
    /// Default: `6`
    pub max_heading_level: usize,

    /// Minimum heading level to create a node.
    /// Headings below this level are treated as content.
    /// Default: `1`
    pub min_heading_level: usize,

    // ============================================================
    // Content Extraction
    // ============================================================

    /// Include code blocks in node content.
    /// Default: `true`
    pub include_code_blocks: bool,

    /// Include images (alt text) in content.
    /// Default: `true`
    pub include_images: bool,

    /// Include links in content.
    /// Default: `true`
    pub include_links: bool,

    /// Include tables in content.
    /// Default: `true`
    pub include_tables: bool,

    // ============================================================
    // Frontmatter
    // ============================================================

    /// Parse YAML frontmatter (`---` delimiters).
    /// Default: `true`
    pub parse_frontmatter: bool,

    /// Parse TOML frontmatter (`+++` delimiters).
    /// Default: `false`
    pub parse_toml_frontmatter: bool,

    /// Fields to extract from frontmatter as metadata.
    /// Default: `["title", "description"]`
    pub frontmatter_fields: Vec<String>,

    // ============================================================
    // Advanced Options
    // ============================================================

    /// Minimum characters required for a heading title to be valid.
    /// Headings with shorter titles are skipped.
    /// Default: `1`
    pub min_heading_chars: usize,

    /// Create an implicit root node for content before the first heading.
    /// Default: `true`
    pub create_preamble_node: bool,

    /// Title for the preamble node (if created).
    /// Default: `"Introduction"`
    pub preamble_title: String,
}

impl Default for MarkdownConfig {
    fn default() -> Self {
        Self {
            // Parsing options - GFM by default (most common)
            enable_gfm: true,
            enable_footnotes: false,
            enable_definition_lists: false,
            enable_super_sub: false,
            max_heading_level: 6,
            min_heading_level: 1,

            // Content extraction - include all by default
            include_code_blocks: true,
            include_images: true,
            include_links: true,
            include_tables: true,

            // Frontmatter
            parse_frontmatter: true,
            parse_toml_frontmatter: false,
            frontmatter_fields: vec!["title".into(), "description".into()],

            // Advanced
            min_heading_chars: 1,
            create_preamble_node: true,
            preamble_title: "Introduction".into(),
        }
    }
}

impl MarkdownConfig {
    /// Create a new configuration with defaults.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Configuration optimized for GitHub Flavored Markdown.
    ///
    /// Enables GFM extensions (tables, strikethrough, task lists).
    #[must_use]
    pub fn gfm() -> Self {
        Self::default()
    }

    /// Configuration for strict CommonMark (no extensions).
    #[must_use]
    pub fn commonmark() -> Self {
        Self {
            enable_gfm: false,
            ..Self::default()
        }
    }

    /// Configuration optimized for documentation sites.
    ///
    /// Enables footnotes and definition lists.
    #[must_use]
    pub fn documentation() -> Self {
        Self {
            enable_footnotes: true,
            enable_definition_lists: true,
            ..Self::default()
        }
    }

    /// Configuration that excludes code blocks from content.
    ///
    /// Useful when code blocks are not relevant for retrieval.
    #[must_use]
    pub fn no_code_blocks() -> Self {
        Self {
            include_code_blocks: false,
            ..Self::default()
        }
    }

    /// Set the maximum heading level.
    #[must_use]
    pub fn with_max_heading_level(mut self, level: usize) -> Self {
        self.max_heading_level = level.clamp(1, 6);
        self
    }

    /// Enable or disable code blocks in content.
    #[must_use]
    pub fn with_code_blocks(mut self, include: bool) -> Self {
        self.include_code_blocks = include;
        self
    }

    /// Enable or disable frontmatter parsing.
    #[must_use]
    pub fn with_frontmatter(mut self, parse: bool) -> Self {
        self.parse_frontmatter = parse;
        self
    }

    /// Set the preamble node title.
    #[must_use]
    pub fn with_preamble_title(mut self, title: impl Into<String>) -> Self {
        self.preamble_title = title.into();
        self
    }
}
