// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Document parsing module.
//!
//! This module provides parsers for different document formats.
//! Each parser extracts [`RawNode`]s from documents that can then be
//! organized into a [`DocumentTree`].
//!
//! # Supported Formats
//!
//! - **Markdown** - Full support via [`MarkdownParser`]
//! - **PDF** - Full support via [`PdfParser`] with TOC extraction
//! - **DOCX** - Full support via [`DocxParser`] with heading detection
//! - **HTML** - Full support via [`HtmlParser`] with heading hierarchy
//!
//! # Example
//!
//! ```rust,no_run
//! use vectorless::parser::{DocumentParser, MarkdownParser, DocumentFormat};
//!
//! # #[tokio::main]
//! # async fn main() -> vectorless::Result<()> {
//! // Create a parser
//! let parser = MarkdownParser::new();
//!
//! // Parse content
//! let content = "# Title\n\nContent here.";
//! let result = parser.parse(content).await?;
//!
//! println!("Extracted {} nodes", result.node_count());
//! for node in &result.nodes {
//!     println!("  - {} (level {})", node.title, node.level);
//! }
//! # Ok(())
//! # }
//! ```

mod registry;
mod traits;
mod types;

// Markdown parsing module
pub mod markdown;

// PDF parsing module
pub mod pdf;

// HTML parsing module
pub mod html;

// TOC processing module
pub mod toc;

// DOCX parsing module
pub mod docx;

// Re-export main types
pub use types::{DocumentFormat, DocumentMeta, ParseResult, RawNode};

// Re-export parser trait
pub use traits::DocumentParser;

// Re-export registry and convenience functions
pub use registry::{ParserRegistry, get_parser, get_parser_for_file, parse_content, parse_file};

// Re-export concrete parsers
pub use docx::DocxParser;
pub use html::{HtmlConfig, HtmlParser};
pub use markdown::{MarkdownConfig, MarkdownParser};
pub use pdf::PdfParser;
