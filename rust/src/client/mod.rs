// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! High-level client API for document indexing and retrieval.
//!
//! This module provides the main entry point for using vectorless:
//! - [`Engine`] — The main client for indexing and querying documents
//! - [`EngineBuilder`] — Builder pattern for client configuration
//! - [`IndexContext`] — Unified input for document indexing
//! - [`QueryContext`] — Unified input for document queries
//!
//! # Quick Start
//!
//! ```rust,no_run
//! use vectorless::client::{EngineBuilder, IndexContext, QueryContext};
//!
//! # #[tokio::main]
//! # async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! // Create a client with default settings
//! let client = EngineBuilder::new()
//!     .with_key("sk-...")
//!     .with_model("gpt-4o")
//!     .build()
//!     .await?;
//!
//! // Index a document
//! let result = client.index(IndexContext::from_path("./document.md")).await?;
//! let doc_id = result.doc_id().unwrap();
//!
//! // Query the document
//! let result = client.query(
//!     QueryContext::new("What is this?").with_doc_id(doc_id)
//! ).await?;
//! if let Some(item) = result.single() {
//!     println!("{}", item.content);
//! }
//!
//! // List all documents
//! for doc in client.list().await? {
//!     println!("{}: {}", doc.id, doc.name);
//! }
//! # Ok(())
//! # }
//! ```
//!
//! # Events and Progress
//!
//! Monitor operation progress with events:
//!
//! ```rust,no_run
//! # use vectorless::client::{EngineBuilder, EventEmitter, IndexEvent};
//! # #[tokio::main]
//! # async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let events = EventEmitter::new()
//!     .on_index(|e| match e {
//!         IndexEvent::Complete { doc_id } => println!("Indexed: {}", doc_id),
//!         _ => {}
//!     });
//!
//! let client = EngineBuilder::new()
//!     .with_events(events)
//!     .build()
//!     .await?;
//! # Ok(())
//! # }
//! ```

mod builder;
mod engine;
mod index_context;
mod indexer;
mod query_context;
mod retriever;
mod types;
mod workspace;

// ============================================================
// Main Types
// ============================================================

pub use builder::{BuildError, EngineBuilder};
pub use engine::Engine;

// ============================================================
// Context Types
// ============================================================

pub use index_context::IndexContext;
pub use query_context::QueryContext;

// ============================================================
// Events (re-export from crate::events)
// ============================================================

pub use crate::events::EventEmitter;

// ============================================================
// Result & Info Types
// ============================================================

pub use types::{
    ClientError, DocumentInfo, FailedItem, IndexItem, IndexMode, IndexOptions, IndexResult,
    QueryResult, QueryResultItem,
};

// ============================================================
// Parser Types (needed for IndexContext::from_content)
// ============================================================

pub use crate::index::parse::DocumentFormat;
