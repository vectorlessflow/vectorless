// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Registry module for managing pluggable components.
//!
//! This module provides registries for:
//! - **Parser Registry** — Document parsers for different formats
//! - **Summarizer Registry** — Summarization strategies
//! - **Retriever Registry** — Retrieval strategies

mod parser_registry;
mod summarizer_registry;
mod retriever_registry;

pub use parser_registry::ParserRegistry;
pub use summarizer_registry::SummarizerRegistry;
pub use retriever_registry::RetrieverRegistry;
