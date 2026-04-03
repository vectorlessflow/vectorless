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
//! ## Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                          client                                  │
//! │                    (Vectorless, Builder)                         │
//! └────────────────────────────┬────────────────────────────────────┘
//!                              │
//!           ┌──────────────────┼──────────────────┐
//!           ▼                  ▼                  ▼
//!     ┌──────────┐       ┌───────────┐      ┌──────────┐
//!     │  index   │       │ retrieval │      │ storage  │
//!     │ (write)  │       │  (read)   │      │ (persist)│
//!     └────┬─────┘       └─────┬─────┘      └────┬─────┘
//!          │                   │                 │
//!          └───────────┬───────┘                 │
//!                      ▼                         │
//!                ┌───────────┐                   │
//!                │  domain   │                   │
//!                │(Tree/Node)│                   │
//!                └─────┬─────┘                   │
//!                      │                         │
//!       ┌──────────────┼──────────────┐          │
//!       ▼              ▼              ▼          │
//!  ┌────────┐    ┌──────────┐   ┌────────┐      │
//!  │ parser │    │   llm    │   │ config │◄─────┘
//!  └────────┘    └──────────┘   └────────┘
//! ```
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
//! async fn main() -> vectorless::domain::Result<()> {
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
//! | [`domain`] | Core domain types (`VectorlessTree`, `VectorlessNode`, `NodeId`) |
//! | [`index`] | Document indexing pipeline |
//! | [`retrieval`] | Retrieval strategies and search algorithms |
//! | [`config`] | Configuration management |
//! | [`llm`] | LLM client with retry & fallback |
//! | [`parser`] | Document parsers (Markdown, PDF, DOCX) |
//! | [`storage`] | Workspace persistence |
//! | [`concurrency`] | Rate limiting |

// =============================================================================
// Modules
// =============================================================================

pub mod client;
pub mod config;
pub mod concurrency;
pub mod domain;
pub mod index;
pub mod llm;
pub mod parser;
pub mod retrieval;
pub mod storage;

// =============================================================================
// Re-exports (Convenience API)
// =============================================================================

// Client API (most common entry point)
pub use client::{DocumentInfo, IndexedDocument, Vectorless, VectorlessBuilder};

// Domain types
pub use domain::{
    Error, Result, NodeId, VectorlessNode, VectorlessTree,
    DocumentStructure, StructureNode,
    TocView, TocNode, TocEntry, TocConfig,
    estimate_tokens, estimate_tokens_fast,
};

// Backward compatibility aliases
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

// Indexing
pub use index::{
    PipelineExecutor, PipelineOptions, IndexInput, IndexMode,
    IndexContext, IndexResult, IndexStage, IndexMetrics,
    SummaryStrategy, ChangeDetector, ChangeSet, PartialUpdater,
};
pub use index::pipeline::{PipelineOrchestrator, CustomStageBuilder};

// Retrieval
pub use retrieval::{
    AdaptiveRetriever, Retriever, RetrieverError, RetrieverResult,
    RetrieveOptions, RetrieveResponse, RetrievalResult, RetrievalContext,
    QueryComplexity, StrategyPreference, SufficiencyLevel,
    ContextBuilder, PruningStrategy, TokenEstimation,
    NavigationDecision, NavigationStep, SearchPath,
    format_for_llm, format_for_llm_async, format_tree_for_llm, format_tree_for_llm_async,
};

// Storage
pub use storage::{DocumentMeta as StorageDocumentMeta, PersistedDocument, Workspace};

// Concurrency
pub use concurrency::{ConcurrencyConfig, ConcurrencyController, RateLimiter};
