// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Document retrieval strategies.
//!
//! This module provides retrievers for finding relevant content in document trees:
//! - **LLM Navigation** — Tree traversal guided by LLM decisions
//! - **Context Building** — Assembling results for LLM consumption
//!
//! # Example
//!
//! ```rust,no_run
//! use vectorless::retriever::{LlmNavigator, RetrieveOptions, ContextBuilder};
//! use vectorless::core::DocumentTree;
//!
//! # #[tokio::main]
//! # async fn main() -> vectorless::core::Result<()> {
//! let tree = DocumentTree::new("Root", "Content");
//! let retriever = LlmNavigator::with_defaults();
//! let options = RetrieveOptions::new().with_top_k(5);
//!
//! let results = retriever.retrieve(&tree, "What is this about?", &options).await?;
//!
//! let context = ContextBuilder::new()
//!     .with_max_tokens(4000)
//!     .build(&results);
//!
//! println!("Context: {}", context);
//! # Ok(())
//! # }
//! ```

mod retriever;
mod llm_navigate;
mod context;

// Re-export main types
pub use retriever::{
    RetrieveOptions,
    RetrievalResult,
    NavigationDecision,
    NavigationContext,
};

pub use llm_navigate::LlmNavigator;

pub use context::{
    ContextBuilder,
    format_for_llm,
    format_tree_for_llm,
};
