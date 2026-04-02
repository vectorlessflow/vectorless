// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! # Vectorless
//!
//! **A hierarchical, reasoning-native document intelligence engine.**
//!
//! Replace your vector database with LLM-powered tree navigation.
//! No embeddings. No vector search. Just reasoning.
//!
//! ## Overview
//!
//! Traditional RAG systems chunk documents into flat vectors, losing structure.
//! Vectorless preserves your document's hierarchy and uses an LLM to navigate it —
//! like a human skimming a table of contents, then drilling into relevant sections.
//!
//! ## Features
//!
//! - 🌳 **Tree-Based Indexing** — Documents as hierarchical trees, not flat chunks
//! - 🧠 **LLM Navigation** — Reasoning-based traversal to find relevant content
//! - 🚀 **Zero Infrastructure** — No vector database, no embedding models
//! - 📄 **Multi-Format** — Markdown, PDF, DOCX support
//! - 💾 **Persistent Workspace** — LRU-cached storage with lazy loading
//! - 🔄 **Retry & Fallback** — Resilient LLM calls with automatic recovery
//!
//! ## Quick Start
//!
//! ```rust,no_run
//! use vectorless::{VectorlessBuilder, Vectorless};
//!
//! #[tokio::main]
//! async fn main() -> vectorless::Result<()> {
//!     // Create client
//!     let mut client = VectorlessBuilder::new()
//!         .with_workspace("./workspace")
//!         .build()?;
//!
//!     // Index a document
//!     let doc_id = client.index("./document.md").await?;
//!
//!     // Query with natural language
//!     let result = client.query(&doc_id, "What is this about?").await?;
//!     println!("{}", result.content);
//!
//!     Ok(())
//! }
//! ```
//!
//! ## Modules
//!
//! | Module | Description |
//! |--------|-------------|
//! | [`client`] | High-level API (`Vectorless`, `VectorlessBuilder`) |
//! | [`core`] | Core types (`DocumentTree`, `TreeNode`, `NodeId`) |
//! | [`config`] | Configuration management |
//! | [`llm`] | LLM client with retry & fallback |
//! | [`document`] | Document parsers (Markdown, PDF, DOCX) |
//! | [`indexer`] | Tree building and optimization |
//! | [`retriever`] | Retrieval strategies (LLM navigate, beam search) |
//! | [`ranking`] | Result scoring and merging |
//! | [`storage`] | Workspace persistence |
//! | [`summarizer`] | Summary generation |
//! | [`concurrency`] | Rate limiting |
//!
//! ## Configuration
//!
//! Create `vectorless.toml` in your project root:
//!
//! ```toml
//! [summary]
//! model = "gpt-4o-mini"
//! endpoint = "https://api.openai.com/v1"
//!
//! [retrieval]
//! model = "gpt-4o"
//! retriever_type = "llm_navigate"
//! top_k = 3
//! ```

// =============================================================================
// Modules
// =============================================================================

pub mod client;
pub mod config;
pub mod concurrency;
pub mod core;
pub mod parser;
pub mod llm;
pub mod ranking;
pub mod storage;
pub mod token;

// =============================================================================
// Re-exports (Convenience API)
// =============================================================================

// Client API (most common entry point)
pub use client::{DocumentInfo, IndexedDocument, Vectorless, VectorlessBuilder};

// Core types
pub use core::{
    DocumentStructure, Error, NodeId, Result, StructureNode,
    VectorlessNode, VectorlessTree,
    // Re-export retriever types from core
    Retriever, RetrieverError, RetrieverResult, RetrievalContext,
    RetrieveOptions, RetrieveResponse, RetrievalResult,
    QueryComplexity, StrategyPreference, SufficiencyLevel,
};

// Backward compatibility alias
#[doc(hidden)]
pub type DocumentTree = VectorlessTree;

#[doc(hidden)]
pub type TreeNode = VectorlessNode;

// Configuration
pub use config::{Config, ConfigLoader, RetrievalConfig, SummaryConfig};

// LLM
pub use llm::{LlmClient, LlmConfig, LlmConfigs, LlmError, LlmPool, RetryConfig};

// Document parsing
pub use parser::{DocumentFormat, DocumentParser, DocxParser, MarkdownParser, PdfParser, ParseResult, RawNode};

// Indexing (use core::index)
pub use core::index::{PipelineExecutor, PipelineOptions, IndexInput, IndexMode};

// Retrieval (from core::retriever)
pub use core::retriever::{
    AdaptiveRetriever, ContextBuilder,
    NavigationDecision, NavigationStep, SearchPath,
    format_for_llm, format_tree_for_llm,};

// Ranking
pub use ranking::{MergeStrategy, Merger, ScoredResult, Scorer, ScoringStrategy};

// Storage
pub use storage::{DocumentMeta as StorageDocumentMeta, PersistedDocument, Workspace};

// Concurrency
pub use concurrency::{ConcurrencyConfig, ConcurrencyController, RateLimiter};

// Token estimation
pub use token::{estimate_tokens, estimate_tokens_fast};
