// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Production-ready Markdown parser module.
//!
//! This module provides a robust Markdown parser built on `pulldown-cmark`,
//! supporting CommonMark, GFM extensions, and frontmatter extraction.
//!
//! # Features
//!
//! - **CommonMark compliant** - Full CommonMark specification support
//! - **GFM extensions** - Tables, strikethrough, task lists, autolinks
//! - **Frontmatter** - YAML and TOML frontmatter parsing
//! - **Configurable** - Fine-grained control over parsing behavior
//!
//! # Example
//!
//! ```rust,ignore
//! use vectorless_compiler::parse::markdown::MarkdownParser;
//!
//! let parser = MarkdownParser::new();
//! ```

mod config;
mod frontmatter;
mod parser;

pub use parser::MarkdownParser;
