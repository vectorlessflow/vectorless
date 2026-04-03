// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! High-level client API for document indexing and retrieval.
//!
//! This module provides the main entry point for using vectorless:
//! - [`Engine`] — The main client for indexing and querying documents
//! - [`EngineBuilder`] — Builder pattern for client configuration
//!
//! # Quick Start
//!
//! ```rust,no_run
//! use vectorless::client::{Engine, EngineBuilder};
//!
//! # #[tokio::main]
//! # async fn main() -> vectorless::domain::Result<()> {
//! // Create a client with default settings
//! let client = Engine::new()?;
//!
//! // Or use the builder for custom configuration
//! let client = EngineBuilder::new()
//!     .with_api_key("your-api-key")
//!     .with_workspace("./my_workspace")
//!     .build()?;
//!
//! // Index a document
//! let doc_id = client.index("./document.md").await?;
//!
//! // Get document structure
//! let structure = client.get_structure(&doc_id)?;
//!
//! // List all documents
//! for doc in client.list_documents() {
//!     println!("{}: {}", doc.id, doc.name);
//! }
//! # Ok(())
//! # }
//! ```
//!
//! # Features
//!
//! - **Document Indexing** — Parse and index Markdown, PDF, and text files
//! - **Tree-Based Structure** — Documents organized as hierarchical trees
//! - **Workspace Persistence** — Save and load indexed documents
//! - **Builder Pattern** — Flexible client configuration

mod types;
mod builder;
mod engine;

// Re-export main types
pub use types::{
    IndexedDocument,
    IndexMode,
    IndexOptions,
    PageContent,
    QueryResult,
    DocumentInfo,
};

pub use builder::{EngineBuilder, BuildError};
pub use engine::Engine;
