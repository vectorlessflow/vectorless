// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Document parsing module (re-export shim).
//!
//! The parser implementation now lives in [`crate::index::parse`].
//! This module re-exports public types for backward compatibility.

// Re-export core types
pub use crate::index::parse::{
    format_from_extension, parse_bytes, parse_content, parse_file, DocumentFormat, DocumentMeta,
    ParseResult, RawNode,
};

// Re-export concrete parsers
pub use crate::index::parse::markdown::{MarkdownConfig, MarkdownParser};
pub use crate::index::parse::pdf::{PdfParser, PdfParserConfig};
