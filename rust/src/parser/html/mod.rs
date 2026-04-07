// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! HTML document parser.
//!
//! This module provides an HTML parser that extracts hierarchical structure
//! from HTML documents using heading tags (`<h1>`-`<h6>`) as section markers.
//!
//! # Features
//!
//! - Parses HTML5 documents using `scraper`
//! - Extracts heading hierarchy (`<h1>`-`<h6>`)
//! - Extracts content from paragraphs, lists, tables, etc.
//! - Preserves document structure
//!
//! # Example
//!
//! ```rust
//! use vectorless::parser::html::HtmlParser;
//! use vectorless::parser::DocumentParser;
//!
//! # #[tokio::main]
//! # async fn main() -> vectorless::Result<()> {
//! let parser = HtmlParser::new();
//! let html = r#"
//! <html>
//! <body>
//!   <h1>Title</h1>
//!   <p>Introduction paragraph.</p>
//!   <h2>Section 1</h2>
//!   <p>Content for section 1.</p>
//! </body>
//! </html>
//! "#;
//! let result = parser.parse(html).await?;
//! println!("Found {} nodes", result.node_count());
//! # Ok(())
//! # }
//! ```

mod config;
mod parser;

pub use config::HtmlConfig;
pub use parser::HtmlParser;
