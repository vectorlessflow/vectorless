// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Storage module for persisting document indices.
//!
//! This module provides:
//! - **Workspace** — A directory-based document collection manager with LRU cache
//! - **Persistence** — Save/load document trees and metadata
//!
//! # Example
//!
//! ```rust,no_run
//! use vectorless::storage::{Workspace, PersistedDocument, DocumentMeta};
//! use vectorless::domain::DocumentTree;
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

mod persistence;
mod workspace;

// Re-export main types
pub use persistence::{
    DocumentMeta,
    PersistedDocument,
    PageContent,
    save_document,
    load_document,
    save_index,
    load_index,
};

pub use workspace::{Workspace, DocumentMetaEntry};
