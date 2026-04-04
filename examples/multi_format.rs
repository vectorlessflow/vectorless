// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Multi-format document processing example.
//!
//! This example demonstrates how to work with different
//! document formats (Markdown, PDF, DOCX, HTML).
//!
//! # What you'll learn:
//! - How to index documents of different formats
//! - How format detection works
//! - How to configure format-specific parsing options
//! - How to handle mixed-format document sets
//!
//! # Supported formats:
//! - **Markdown** (.md): Full support with ToC extraction
//! - **PDF** (.pdf): Text extraction, structure inference
//! - **DOCX** (.docx): Word document parsing
//! - **HTML** (.html, .htm): Web page parsing (planned)
//! - **Plain text** (.txt): Basic text parsing (planned)
//!
//! # Format-specific considerations:
//!
//! ## Markdown
//! - Best format for structured documents
//! - Automatic heading hierarchy detection
//! - Code block handling
//!
//! ## PDF
//! - Text extraction quality varies
//! - No explicit structure (inferred from fonts/spacing)
//! - Tables and images not supported
//!
//! ## DOCX
//! - Good structure preservation
//! - Styles mapped to hierarchy
//! - Limited formatting support
//!
//! # TODO: Implementation steps
//!
//! 1. Detect document format from extension or content
//! 2. Configure format-specific parser options
//! 3. Index documents of mixed formats
//! 4. Query across all formats

// TODO: Implement multi-format example
// ```
// use vectorless::client::{Engine, EngineBuilder};
// use vectorless::parser::DocumentFormat;
//
// async fn index_multiple_formats(engine: &Engine) {
//     // Index different formats
//     let md_doc = engine.index("./README.md").await?;
//     let pdf_doc = engine.index("./paper.pdf").await?;
//     let docx_doc = engine.index("./report.docx").await?;
//
//     // Query works across all formats
//     let result = engine.query(&md_doc, "What is this about?").await?;
// }
// ```

fn main() {
    // TODO: Show multi-format indexing and querying
    //
    // // Index documents of different formats
    // let md_id = engine.index("./docs/guide.md").await?;
    // let pdf_id = engine.index("./docs/paper.pdf").await?;
    // let docx_id = engine.index("./docs/report.docx").await?;
    //
    // // Each can be queried independently
    // for doc_id in &[md_id, pdf_id, docx_id] {
    //     let result = engine.query(doc_id, "summary").await?;
    //     println!("Result: {}", result.content);
    // }

    println!("TODO: Implement multi_format example");
}
