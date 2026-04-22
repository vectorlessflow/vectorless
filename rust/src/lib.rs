// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

#![allow(dead_code)]

//! # Vectorless
//!
//! A Document Understanding Engine for AI.
//!
//! It compiles documents into structured trees of meaning, then dispatches
//! multiple agents to reason through headings, sections, and paragraphs.
//!
//! ## Quick Start
//!
//! ```rust,no_run
//! use vectorless::{EngineBuilder, IngestInput};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let engine = EngineBuilder::new()
//!         .with_key("sk-...")
//!         .with_model("gpt-4o")
//!         .with_endpoint("https://api.openai.com/v1")
//!         .build()
//!         .await?;
//!
//!     // Understand a document
//!     let doc = engine.ingest(IngestInput::Path("./report.pdf".into())).await?;
//!     println!("{}: {}", doc.name, doc.summary);
//!
//!     // Ask a question
//!     let answer = engine.ask("What is the total revenue?", &[doc.doc_id.clone()]).await?;
//!     println!("{}", answer.content);
//!
//!     Ok(())
//! }
//! ```

// ── Modules ──────────────────────────────────────────────────────────────────

mod agent;
mod client;
mod config;
mod document;
mod error;
mod events;
mod graph;
mod metrics;

mod index;
mod llm;
mod query;
mod rerank;
mod retrieval;
mod scoring;
mod storage;
mod utils;

// ── Public API ───────────────────────────────────────────────────────────────

// Client
pub use client::{BuildError, Engine, EngineBuilder};

// Config
pub use config::Config;

// Documents (understanding types)
pub use document::{
    Answer, Concept, Document, DocumentInfo, DocumentStructure, DocumentTree, Evidence,
    IngestInput, NodeId, ReasoningIndexConfig, ReasoningTrace, StructureNode, TocConfig,
    TocEntry, TocNode, TocView, TraceStep, TreeNode,
};

// Graph
pub use graph::{DocumentGraph, DocumentGraphNode, EdgeEvidence, GraphEdge, WeightedKeyword};

// Events
pub use events::{EventEmitter, IndexEvent, QueryEvent, WorkspaceEvent};

// Metrics
pub use metrics::{IndexMetrics, LlmMetricsReport, MetricsReport, RetrievalMetricsReport};

// Errors
pub use error::{Error, Result};

/// Test-only utilities.
///
/// **Do not use in production code.** This module exposes helpers for writing
/// integration tests without a real LLM endpoint.
#[doc(hidden)]
pub mod __test_support {
    pub use crate::client::test_support::*;
}
