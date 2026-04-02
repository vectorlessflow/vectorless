// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! PDF document parsing module.
//!
//! This module provides functionality to parse PDF documents:
//! - **PdfPage** — Single page with text and metadata
//! - **PdfParser** — Extract pages from PDF files
//!
//! # Example
//!
//! ```rust,no_run
//! use vectorless::parser::pdf::{PdfParser, PdfPage};
//! use std::path::Path;
//!
//! # fn main() -> vectorless::core::Result<()> {
//! let parser = PdfParser::new();
//! let result = parser.parse_file(Path::new("document.pdf"))?;
//!
//! println!("Pages: {}", result.pages.len());
//! for page in &result.pages {
//!     println!("Page {}: {} tokens", page.number, page.token_count);
//! }
//! # Ok(())
//! # }
//! ```

mod parser;
mod types;

pub use parser::{PdfParser, PdfParserConfig};
pub use types::{PdfMetadata, PdfPage, PdfParseResult};
