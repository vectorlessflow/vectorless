// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! # Vectorless

// Clippy: allow specific lints that are too noisy for this project
#![allow(clippy::iter_over_hash_type)]
#![allow(clippy::large_enum_variant)]
#![allow(clippy::manual_unwrap_or_default)]

//! # Vectorless
//!
//! An ultra-performant reasoning-native document intelligence engine for AI.
//!
//! It transforms documents into rich semantic trees and uses LLMs to
//! intelligently traverse the hierarchy — retrieving the most relevant content
//! through structural reasoning and deep contextual understanding.
//!
//! ## Quick Start
//!
//! ```rust,no_run
//! use vectorless::{EngineBuilder, IndexContext, QueryContext};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let client = EngineBuilder::new()
//!         .with_workspace("./workspace")
//!         .with_key("sk-...")
//!         .with_model("gpt-4o")
//!         .build()
//!         .await?;
//!
//!     let result = client.index(IndexContext::from_path("./document.md")).await?;
//!     let doc_id = result.doc_id().unwrap();
//!
//!     let result = client.query(
//!         QueryContext::new("What is this about?").with_doc_id(doc_id)
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

// Retrieval types
pub use retrieval::StrategyPreference;
pub use retrieval::pipeline::SearchAlgorithm;
pub use retrieval::QueryComplexity;

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

// Index metrics
pub use metrics::IndexMetrics;
