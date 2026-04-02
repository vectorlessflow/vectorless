// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Table of Contents (TOC) processing module.
//!
//! This module provides functionality to extract and verify document structure
//! from PDF Table of Contents:
//!
//! - **Detection** — Find TOC in document (regex + LLM fallback)
//! - **Parsing** — Convert TOC text to structured entries (LLM)
//! - **Assignment** — Map TOC pages to physical pages
//! - **Verification** — Sample verification of page assignments
//! - **Repair** — Fix incorrect assignments
//!
//! # Architecture
//!
//! ```text
//! PDF Pages
//!     │
//!     ▼
//! ┌─────────────────────────────────────────────────┐
//! │              TocProcessor                        │
//! │                                                  │
//! │  ┌─────────┐  ┌─────────┐  ┌─────────┐         │
//! │  │Detector │─▶│ Parser  │─▶│Assigner │         │
//! │  └─────────┘  └─────────┘  └────┬────┘         │
//! │                                │                │
//! │                                ▼                │
//! │                         ┌─────────────┐         │
//! │                         │  Verifier   │         │
//! │                         └──────┬──────┘         │
//! │                                │                │
//! │                                ▼                │
//! │                         ┌─────────────┐         │
//! │                         │  Repairer   │         │
//! │                         └─────────────┘         │
//! └─────────────────────────────────────────────────┘
//!     │
//!     ▼
//! Vec<TocEntry>
//! ```
//!
//! # Example
//!
//! ```rust,no_run
//! use vectorless::parser::toc::TocProcessor;
//! use vectorless::parser::pdf::{PdfParser, PdfPage};
//!
//! # #[tokio::main]
//! # async fn main() -> vectorless::core::Result<()> {
//! // Parse PDF
//! let pdf_parser = PdfParser::new();
//! let result = pdf_parser.parse_file("document.pdf".as_ref())?;
//!
//! // Extract TOC
//! let processor = TocProcessor::new();
//! let entries = processor.process(&result.pages).await?;
//!
//! // Use entries
//! for entry in &entries {
//!     println!("{} - Page {:?}", entry.title, entry.physical_page);
//! }
//! # Ok(())
//! # }
//! ```

mod types;
mod detector;
mod parser;
mod assigner;
mod verifier;
mod repairer;
mod processor;

// Re-export main types
pub use types::{TocEntry, TocDetection, PageOffset, VerificationError, VerificationReport, ErrorType};

// Re-export components
pub use detector::{TocDetector, TocDetectorConfig};
pub use parser::{TocParser, TocParserConfig};
pub use assigner::{PageAssigner, PageAssignerConfig};
pub use verifier::{IndexVerifier, VerifierConfig};
pub use repairer::{IndexRepairer, RepairerConfig};
pub use processor::{TocProcessor, TocProcessorConfig};
