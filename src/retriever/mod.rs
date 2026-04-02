// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Document retrieval strategies.
//!
//! This module provides retrievers for finding relevant content in document trees:
//! - **Adaptive Retriever** — Automatically selects best strategy based on query
//! - **LLM Navigation** — Tree traversal guided by LLM decisions
//! - **Context Building** — Assembling results for LLM consumption
//!
//! # Example
//!
//! ```rust,no_run
//! use vectorless::retriever::{AdaptiveRetriever, RetrieveOptions};
//! use vectorless::core::VectorlessTree;
//!
//! # #[tokio::main]
//! # async fn main() -> vectorless::core::Result<()> {
//! let tree = VectorlessTree::new("Root", "Content");
//! let retriever = AdaptiveRetriever::new();
//! let options = RetrieveOptions::default().with_top_k(5);
//!
//! let response = retriever.retrieve(&tree, "What is this about?", &options).await?;
//! println!("Found {} results", response.results.len());
//! # Ok(())
//! # }
//! ```

// Re-export from core::retriever (new unified module)
pub use crate::core::retriever::{
    // Main types
    AdaptiveRetriever,
    Retriever,
    RetrieverError,
    RetrieverResult,
    RetrievalContext,
    RetrieveOptions,
    RetrieveResponse,
    RetrievalResult,
    QueryComplexity,
    StrategyPreference,
    SufficiencyLevel,
    NavigationDecision,
    NavigationStep,
    SearchPath,
};

// Context builder for formatting results
mod context;
pub use context::{
    ContextBuilder,
    format_for_llm,
    format_tree_for_llm,
};

// LLM Navigator (concrete implementation)
mod llm_navigate;
pub use llm_navigate::LlmNavigator;
