// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Storage module for persisting document indices.
//!
//! This module provides:
//! - **Workspace** — An async directory-based document collection manager with LRU cache
//! - **Persistence** — Save/load document trees and metadata with atomic writes
//! - **Cache** — LRU cache for loaded documents
//! - **Lock** — File locking for multi-process safety
//! - **Backend** — Storage backend abstraction (file, memory, etc.)
//!
//! # Example
//!
//! ```rust,no_run
//! use vectorless::storage::{Workspace, PersistedDocument, DocumentMeta};
//! use vectorless::document::DocumentTree;
//!
//! # #[tokio::main]
//! # async fn main() -> vectorless::error::Result<()> {
//! // Create a workspace
//! let workspace = Workspace::new("./my_workspace").await?;
//!
//! // Add a document
//! let meta = DocumentMeta::new("doc-1", "My Document", "md");
//! let tree = DocumentTree::new("Root", "Content");
//! let doc = PersistedDocument::new(meta, tree);
//! workspace.add(&doc).await?;
//!
//! // Load it back (uses LRU cache)
//! let loaded = workspace.load_and_cache("doc-1").await?.unwrap();
//! # Ok(())
//! # }
//! ```

pub mod backend;
pub mod cache;
pub mod codec;
pub mod lock;
pub mod migration;
mod persistence;
pub mod workspace;

// Re-export main types
pub use persistence::{DocumentMeta, PersistedDocument};
pub use workspace::Workspace;
