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
//! │                     (Engine, EngineBuilder)                      │
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
//! use vectorless::{EngineBuilder, Engine};
//!
//! #[tokio::main]
//! async fn main() -> vectorless::domain::Result<()> {
//!     // Create client
//!     let mut client = EngineBuilder::new()
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
//! | [`client`] | High-level API (`Engine`, `EngineBuilder`) |
//! | [`domain`] | Core domain types (`DocumentTree`, `TreeNode`, `NodeId`) |
//! | [`index`] | Document indexing pipeline |
//! | [`retrieval`] | Retrieval strategies and search algorithms |
//! | [`config`] | Configuration management |
//! | [`llm`] | LLM client with retry & fallback |
//! | [`parser`] | Document parsers (Markdown, PDF, DOCX) |
//! | [`storage`] | Workspace persistence |
//! | [`throttle`] | Rate limiting |

// =============================================================================
// Modules
// =============================================================================

pub mod client;
pub mod config;
pub mod throttle;
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
pub use client::{DocumentInfo, IndexedDocument, Engine, EngineBuilder};

// Domain types
pub use domain::{
    Error, Result, NodeId, TreeNode, DocumentTree,
    DocumentStructure, StructureNode,
    TocView, TocNode, TocEntry, TocConfig,
    estimate_tokens, estimate_tokens_fast,
};

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

// Throttle
pub use throttle::{ConcurrencyConfig, ConcurrencyController, RateLimiter};
