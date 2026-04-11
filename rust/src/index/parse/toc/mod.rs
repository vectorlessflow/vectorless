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

mod assigner;
mod detector;
mod parser;
mod processor;
mod repairer;
mod types;
mod verifier;

// Re-export main types
pub use types::TocEntry;

// Re-export components
pub use processor::TocProcessor;
