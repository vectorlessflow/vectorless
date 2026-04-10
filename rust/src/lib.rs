// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! # Vectorless

// Clippy: allow some pedantic lints that are too noisy for early-stage project
#![allow(clippy::all)]
#![allow(dead_code)]
#![allow(unused_variables)]
#![allow(unused_imports)]
#![allow(clippy::iter_over_hash_type)]
#![allow(clippy::large_enum_variant)]
#![allow(clippy::manual_unwrap_or_default)]

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
//!                              ┌─────────────────────────────────────────────────┐
//!                              │                    USER                          │
//!                              │              (Query / Index)                     │
//!                              └────────────────────────┬────────────────────────┘
//!                                                       │
//!                                                       ▼
//! ┌─────────────────────────────────────────────────────────────────────────────────┐
//! │                              CLIENT LAYER                                        │
//! │  ┌───────────────────────────────────────────────────────────────────────────┐  │
//! │  │                           Engine / EngineBuilder                            │  │
//! │  │                    (Unified API for Index + Query)                          │  │
//! │  └───────────────────────────────────────────────────────────────────────────┘  │
//! └─────────────────────────────────────────────────────────────────────────────────┘
//!                                                       │
//!                              ┌────────────────────────┴────────────────────────┐
//!                              │                                                 │
//!                              ▼                                                 ▼
//! ┌──────────────────────────────────────────────┐   ┌──────────────────────────────────────────────┐
//! │              INDEX PIPELINE                   │   │              RETRIEVAL ENGINE                │
//! │  ┌─────────┐  ┌─────────┐  ┌─────────────┐   │   │  ┌─────────────────────────────────────┐    │
//! │  │  Parse  │─▶│  Build  │─▶│   Enhance   │   │   │  │           Pilot (LLM)               │    │
//! │  │ (Doc)   │  │ (Tree)  │  │ (Summaries) │   │   │  │     ┌───────────────────────┐       │    │
//! │  └─────────┘  └────┬────┘  └──────┬──────┘   │   │  │     │   Navigation Agent    │       │    │
//! │       │            │              │          │   │  │     │  ┌─────┐ ┌─────────┐  │       │    │
//! │       ▼            ▼              ▼          │   │  │     │  │Decide│▶│Traverse │  │       │    │
//! │  ┌─────────┐  ┌─────────┐  ┌─────────────┐   │   │  │     │  │Path │ │  Tree   │  │       │    │
//! │  │ Enrich  │─▶│ Optimize│─▶│   Persist   │   │   │  │     │  └─────┘ └─────────┘  │       │    │
//! │  │(Meta)   │  │ (Tree)  │  │  (Storage)  │   │   │  │     └───────────────────────┘       │    │
//! │  └─────────┘  └─────────┘  └─────────────┘   │   │  └─────────────────────────────────────┘    │
//! │       │                                        │   │                    │                        │
//! │       │         ┌──────────────────────┐      │   │                    ▼                        │
//! │       └────────▶│   Change Detector    │◀─────┼───┤  ┌─────────────────────────────────────┐    │
//! │                  │  (Fingerprint-based) │      │   │  │         Context Assembler           │    │
//! │                  └──────────────────────┘      │   │  │   ┌─────────┐ ┌─────────────────┐  │    │
//! │                                                │   │  │   │ Pruning │ │  Token Budget   │  │    │
//! └──────────────────────────────────────────────┘   │  │   │Strategy │ │    Management   │  │    │
//!                              │                     │  │   └─────────┘ └─────────────────┘  │    │
//!                              │                     │  └─────────────────────────────────────┘    │
//!                              │                     │                    │                        │
//!                              ▼                     │                    ▼                        │
//! ┌──────────────────────────────────────────────────────────────────────────────────────────────┐
//! │                                  DOMAIN LAYER (Core)                                          │
//! │                                                                                               │
//! │   ┌───────────────────┐     ┌───────────────────┐     ┌───────────────────┐                   │
//! │   │   DocumentTree    │     │    TreeNode       │     │    NodeId         │                   │
//! │   │   (Arena-based)   │────▶│  - title          │     │   (indextree)     │                   │
//! │   │                   │     │  - content        │     │                   │                   │
//! │   └───────────────────┘     │  - summary        │     └───────────────────┘                   │
//! │            │                │  - depth          │              │                              │
//! │            ▼                │  - token_count    │              │                              │
//! │   ┌───────────────────┐     └───────────────────┘              │                              │
//! │   │     TocView       │              │                         │                              │
//! │   │  (Table of        │              │                         │                              │
//! │   │   Contents)       │              │                         │                              │
//! │   └───────────────────┘              │                         │                              │
//! └──────────────────────────────────────────────────────────────────────────────────────────────┘
//!                                        │                         │
//!                      ┌─────────────────┴─────────────────────────┴─────────────────┐
//!                      │                                                               │
//!                      ▼                                                               ▼
//! ┌─────────────────────────────────────────┐   ┌─────────────────────────────────────────────────┐
//! │            SUPPORT LAYER                 │   │                 STORAGE LAYER                   │
//! │                                          │   │                                                  │
//! │  ┌─────────────┐  ┌──────────────────┐   │   │  ┌────────────────┐  ┌─────────────────────┐    │
//! │  │    LLM      │  │     Parser       │   │   │  │   Workspace    │  │    MemoStore        │    │
//! │  │  (OpenAI)   │  │ - Markdown       │   │   │  │  (Persistence) │  │  (LLM Cache)        │    │
//! │  │             │  │ - PDF            │   │   │  │                │  │  - LRU Eviction     │    │
//! │  │ ┌─────────┐ │  │ - DOCX           │   │   │  │ ┌────────────┐ │  │  - TTL Expiration   │    │
//! │  │ │  Pool   │ │  │                  │   │   │  │ │   LRU     │ │  │  - Disk Persist     │    │
//! │  │ │ Retry   │ │  └──────────────────┘   │   │  │ │   Cache   │ │  │                      │    │
//! │  │ │ Fallback│ │                          │   │  │ └────────────┘ │  └─────────────────────┘    │
//! │  │ └─────────┘ │  ┌──────────────────┐   │   │  │                │                               │
//! │  └─────────────┘  │   Fingerprint    │   │   │  │ ┌────────────┐ │  ┌─────────────────────┐    │
//! │                    │   (BLAKE2b)      │   │   │  │ │  Atomic    │ │  │   ChangeDetector    │    │
//! │  ┌─────────────┐  │                  │   │   │  │ │  Writes    │ │  │   (Incremental)     │    │
//! │  │   Config    │  │ ┌──────────────┐ │   │   │  │ └────────────┘ │  │                     │    │
//! │  │   Loader    │  │ │ Content FP   │ │   │   │  │                │  │ ┌─────────────────┐ │    │
//! │  │             │  │ │ Subtree FP   │ │   │   │  └────────────────┘  │ │ Processing Ver  │ │    │
//! │  └─────────────┘  │ │ Node FP      │ │   │   │                      │ └─────────────────┘ │    │
//! │                    │ └──────────────┘ │   │   │                      └─────────────────────┘    │
//! │  ┌─────────────┐  └──────────────────┘   │   │                                                  │
//! │  │  Throttle   │                          │   │  ┌────────────────────────────────────────────┐ │
//! │  │ (Rate Limit)│  ┌──────────────────┐   │   │  │              DocumentMeta                  │ │
//! │  └─────────────┘  │   Throttle       │   │   │  │  - content_fingerprint                     │ │
//! │                    │   (Concurrency)  │   │   │  │  - processing_version                      │ │
//! │                    └──────────────────┘   │   │  │  - node_count, total_summary_tokens        │ │
//! │                                          │   │  └────────────────────────────────────────────┘ │
//! └─────────────────────────────────────────┘   └─────────────────────────────────────────────────┘
//! ```
//!
//! ## Data Flow
//!
//! ### Indexing Flow
//! ```text
//! Document ──▶ Parse ──▶ Build Tree ──▶ Generate Summaries ──▶ Detect Changes ──▶ Persist
//!                            │                │                      │
//!                            │                └──▶ MemoStore ◀───────┘
//!                            │                      (Cache)
//!                            └──▶ Fingerprint ──▶ ChangeDetector
//! ```
//!
//! ### Query Flow
//! ```text
//! Query ──▶ Pilot Agent ──▶ Navigate Tree ──▶ Assemble Context ──▶ Return Result
//!               │                 │                   │
//!               └──▶ LLM ◀────────┘                   │
//!                    (Decide)                         │
//!                                                     └──▶ MemoStore (Cached Summaries)
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
//! - 🔍 **Incremental Updates** — Fingerprint-based change detection
//! - ⚡ **LLM Memoization** — Cache summaries and decisions to reduce costs
//!
//! ## Quick Start
//!
//! ```rust,no_run
//! use vectorless::{EngineBuilder, Engine};
//! use vectorless::client::IndexContext;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Create client
//!     let client = EngineBuilder::new()
//!         .with_workspace("./workspace")
//!         .build()
//!         .await?;
//!
//!     // Index a document
//!     let doc_id = client.index(IndexContext::from_path("./document.md")).await?;
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
//! | [`document`] | Core domain types (`DocumentTree`, `TreeNode`, `NodeId`) |
//! | [`index`] | Document indexing pipeline with incremental updates |
//! | [`retrieval`] | Retrieval strategies and LLM-based navigation |
//! | [`config`] | Configuration management |
//! | [`llm`] | LLM client with retry & fallback |
//! | [`parser`] | Document parsers (Markdown, PDF, DOCX) |
//! | [`storage`] | Workspace persistence with LRU caching |
//! | [`throttle`] | Rate limiting and concurrency control |
//! | [`fingerprint`] | Content and subtree fingerprinting |
//! | [`memo`] | LLM result memoization and caching |

// =============================================================================
// Modules
// =============================================================================

pub mod client;
pub mod config;
pub mod document;
pub mod error;
pub mod index;
pub mod llm;
pub mod memo;
pub mod metrics;
pub mod parser;
pub mod retrieval;
pub mod storage;
pub mod throttle;
pub mod utils;

// =============================================================================
// Re-exports (Convenience API)
// =============================================================================

// Client API (most common entry point)
pub use client::{
    BuildError, DocumentInfo, Engine, EngineBuilder, IndexContext, IndexMode, IndexOptions,
    IndexSource, IndexedDocument,
};

// Error types
pub use error::{Error, Result};

// Document types
pub use document::{
    DocumentStructure, DocumentTree, NodeId, StructureNode, TocConfig, TocEntry, TocNode, TocView,
    TreeNode,
};

// Utility functions
pub use utils::{estimate_tokens, estimate_tokens_fast};

// Configuration
pub use config::{Config, ConfigLoader, RetrievalConfig, SummaryConfig};

// LLM
pub use llm::{LlmClient, LlmConfig, LlmConfigs, LlmError, LlmPool, RetryConfig};

// Document parsing
pub use parser::{
    DocumentFormat, DocumentParser, DocxParser, MarkdownParser, ParseResult, PdfParser, RawNode,
};

// Indexing
pub use index::pipeline::{CustomStageBuilder, PipelineOrchestrator};
pub use index::{
    ChangeDetector, ChangeSet, IndexContext as PipelineIndexContext, IndexInput, IndexMetrics,
    IndexMode as PipelineIndexMode, IndexResult, IndexStage, PartialUpdater, PipelineExecutor,
    PipelineOptions, SummaryStrategy,
};

// Retrieval
pub use retrieval::{
    ContextBuilder, NavigationDecision, NavigationStep, PipelineRetriever, PruningStrategy,
    QueryComplexity, RetrievalContext, RetrievalResult, RetrieveEvent, RetrieveOptions,
    RetrieveResponse, Retriever, RetrieverError, RetrieverResult, SearchPath, StrategyPreference,
    SufficiencyLevel, TokenEstimation, format_for_llm, format_for_llm_async, format_tree_for_llm,
    format_tree_for_llm_async,
};

// Storage
pub use storage::{DocumentMeta as StorageDocumentMeta, PersistedDocument, Workspace};

// Throttle
pub use throttle::{ConcurrencyConfig, ConcurrencyController, RateLimiter};

// Memo
pub use memo::{MemoEntry, MemoKey, MemoOpType, MemoStats, MemoStore, MemoValue};
