// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0
#![allow(dead_code)]

//! # Vectorless
//!
//! A document engine for AI. It transforms documents into hierarchical semantic
//! trees and uses the LLM itself to navigate and retrieve — purely LLM-guided,
//! from indexing to querying. No vector databases, no embeddings, no similarity search.
//!
//! ## Quick Start
//!
//! ```rust,no_run
//! use vectorless::{EngineBuilder, IndexContext, QueryContext};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let client = EngineBuilder::new()
//!         .with_key("sk-...")
//!         .with_model("gpt-4o")
//!         .build()
//!         .await?;
//!
//!     let result = client.index(IndexContext::from_path("./document.md")).await?;
//!     let doc_id = result.doc_id().unwrap();
//!
//!     let result = client.query(
//!         QueryContext::new("What is this about?").with_doc_ids(vec![doc_id.to_string()])
//!     ).await?;
//!     println!("{}", result.content);
//!
//!     Ok(())
//! }
//! ```

pub mod client;
mod config;
pub mod document;
pub mod error;
pub mod events;
pub mod graph;
mod index;
mod llm;
mod memo;
pub mod metrics;
mod retrieval;
mod storage;
mod throttle;
mod utils;

// Client API
pub use client::{
    BuildError, ClientError, DocumentFormat, DocumentInfo, Engine, EngineBuilder, FailedItem,
    IndexContext, IndexItem, IndexMode, IndexOptions, IndexResult, QueryContext, QueryResult,
    QueryResultItem,
};

// Error types
pub use error::{Error, Result};

// Document types
pub use document::{
    DocumentStructure, DocumentTree, NodeId, ReasoningIndexConfig, StructureNode, TocConfig,
    TocEntry, TocNode, TocView, TreeNode,
};

// Graph types
pub use graph::DocumentGraph;

// Event types
pub use events::{EventEmitter, IndexEvent, QueryEvent, WorkspaceEvent};

// Runtime metrics reports
pub use metrics::{
    IndexMetrics, LlmMetricsReport, MetricsReport, PilotMetricsReport, RetrievalMetricsReport,
};
