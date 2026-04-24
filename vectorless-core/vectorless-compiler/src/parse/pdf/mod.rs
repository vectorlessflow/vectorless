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
//! ```rust,ignore
//! use vectorless_compiler::parse::pdf::{PdfParser, PdfPage};
//! use std::path::Path;
//!
//! let parser = PdfParser::new();
//! let result = parser.parse_file(Path::new("document.pdf"))?;
//!
//! println!("Pages: {}", result.pages.len());
//! for page in &result.pages {
//!     println!("Page {}: {} tokens", page.number, page.token_count);
//! }
//! ```

mod parser;
mod types;

pub use parser::PdfParser;
pub use types::PdfPage;
