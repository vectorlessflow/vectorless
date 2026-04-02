// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! DOCX document parsing module.
//!
//! This module provides functionality to parse DOCX (Microsoft Word) documents:
//! - **DocxParser** — Extract structured content from DOCX files
//! - **StyleResolver** — Resolve heading styles from style definitions
//!
//! # DOCX Structure
//!
//! A DOCX file is a ZIP archive containing XML files:
//!
//! ```text
//! document.docx
//! ├── word/
//! │   ├── document.xml   # Main content
//! │   └── styles.xml     # Style definitions (optional)
//! ```
//!
//! # Example
//!
//! ```rust,no_run
//! use vectorless::parser::docx::DocxParser;
//! use vectorless::core::DocumentParser;
//! use std::path::Path;
//!
//! # #[tokio::main]
//! # async fn main() -> vectorless::core::Result<()> {
//! let parser = DocxParser::new();
//! let result = parser.parse_file(Path::new("document.docx")).await?;
//!
//! println!("Extracted {} nodes", result.node_count());
//! for node in &result.nodes {
//!     println!("  - {} (level {})", node.title, node.level);
//! }
//! # Ok(())
//! # }
//! ```

mod parser;
mod styles;
mod types;

pub use parser::DocxParser;
pub use styles::StyleResolver;
pub use types::{DocxParagraph, DocxStyle};
