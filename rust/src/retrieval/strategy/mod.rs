// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Retrieval strategies for different query types.
//!
//! This module provides several retrieval strategies:
//!
//! - **KeywordStrategy**: Fast keyword matching using TF-IDF
//! - **LlmStrategy**: LLM-powered reasoning with ToC context
//! - **HybridStrategy**: BM25 pre-filter + LLM refinement (recommended)
//! - **CrossDocumentStrategy**: Multi-document retrieval with result aggregation
//! - **PageRangeStrategy**: Filter by page range before retrieval

mod cross_document;
mod hybrid;
mod keyword;
mod llm;
mod page_range;
mod r#trait;

pub use cross_document::{CrossDocumentConfig, CrossDocumentStrategy, DocumentEntry};
pub use hybrid::{HybridConfig, HybridStrategy};
pub use keyword::KeywordStrategy;
pub use llm::LlmStrategy;
pub use r#trait::RetrievalStrategy;
