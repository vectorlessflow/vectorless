// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

#![allow(dead_code)]

//! # Vectorless
//!
//! A reasoning-native document engine for AI.
//!
//! It will reason through any of your structured documents — **PDFs, Markdown,
//! reports, contracts** — and retrieve only what's relevant. Nothing more,
//! nothing less.
//!
//! ## Quick Start
//!
//! ```rust,no_run
//! use vectorless::{EngineBuilder, IndexContext, QueryContext};
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
//!     let result = engine.index(IndexContext::from_path("./document.md")).await?;
//!     let doc_id = result.doc_id().unwrap();
//!
//!     let result = engine.query(
//!         QueryContext::new("What is this about?")
//!             .with_doc_ids(vec![doc_id.to_string()]),
//!     ).await?;
//!     println!("{}", result.content);
//!
//!     Ok(())
//! }
//! ```

// ── Modules ──────────────────────────────────────────────────────────────────

mod client;
mod config;
mod document;
mod error;
mod events;
mod graph;
mod metrics;

mod index;
mod llm;
mod memo;
mod retrieval;
mod storage;
mod throttle;
mod utils;

// ── Public API ───────────────────────────────────────────────────────────────

// Client
pub use client::{
    BuildError, DocumentFormat, DocumentInfo, Engine, EngineBuilder, FailedItem,
    IndexContext, IndexItem, IndexMode, IndexOptions, IndexResult, QueryContext, QueryResult,
    QueryResultItem,
};

// Config
pub use config::Config;

// Documents
pub use document::{
    DocumentStructure, DocumentTree, NodeId, ReasoningIndexConfig, StructureNode, TocConfig,
    TocEntry, TocNode, TocView, TreeNode,
};

// Graph
pub use graph::{DocumentGraph, DocumentGraphNode, EdgeEvidence, GraphEdge, WeightedKeyword};

// Events
pub use events::{EventEmitter, IndexEvent, QueryEvent, WorkspaceEvent};

// Metrics
pub use metrics::{
    IndexMetrics, LlmMetricsReport, MetricsReport, PilotMetricsReport, RetrievalMetricsReport,
};

// Errors
pub use error::{Error, Result};
