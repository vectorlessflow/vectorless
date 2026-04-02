// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Registry module for managing pluggable components.
//!
//! This module provides registries for:
//! - **Parser Registry** — Document parsers for different formats
//! - **Summarizer Registry** — Summarization strategies

mod parser_registry;
mod summarizer_registry;

pub use parser_registry::ParserRegistry;
pub use summarizer_registry::SummarizerRegistry;
