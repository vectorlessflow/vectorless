// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Storage module for persisting document indices.
//!
//! This module provides:
//! - **Workspace** — A directory-based document collection manager with LRU cache
//! - **Persistence** — Save/load document trees and metadata with atomic writes
//! - **Cache** — LRU cache for loaded documents
//! - **Lock** — File locking for multi-process safety
//!
//! # Example
//!
//! ```rust,no_run
//! use vectorless::storage::{Workspace, PersistedDocument, DocumentMeta};
//! use vectorless::document::DocumentTree;
//!
//! // Create a workspace
//! let mut workspace = Workspace::new("./my_workspace")?;
//!
//! // Add a document
//! let meta = DocumentMeta::new("doc-1", "My Document", "md");
//! let tree = DocumentTree::new("Root", "Content");
//! let doc = PersistedDocument::new(meta, tree);
//! workspace.add(&doc)?;
//!
//! // Load it back (uses LRU cache)
//! let loaded = workspace.load("doc-1")?.unwrap();
//! ```

pub mod cache;
pub mod lock;
mod persistence;
mod workspace;

// Re-export main types
pub use cache::DocumentCache;
pub use lock::{FileLock, ScopedLock};
pub use persistence::{
    DocumentMeta, PageContent, PersistedDocument,
    load_document, load_document_with_options, load_index, load_index_with_options,
    save_document, save_document_with_options, save_index, save_index_with_options,
    PersistenceOptions,
};
pub use workspace::{DocumentMetaEntry, Workspace, WorkspaceOptions};
