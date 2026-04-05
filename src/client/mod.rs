// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! High-level client API for document indexing and retrieval.
//!
//! This module provides the main entry point for using vectorless:
//! - [`Engine`] — The main client for indexing and querying documents
//! - [`EngineBuilder`] — Builder pattern for client configuration
//! - [`IndexContext`] — Unified input for document indexing
//! - [`Session`] — Multi-document session management
//!
//! # Architecture
//!
//! The client module is organized into specialized sub-modules:
//!
//! ```text
//! client/
//! ├── mod.rs           → Re-exports and documentation
//! ├── engine.rs        → Main orchestrator
//! ├── builder.rs       → Builder pattern
//! ├── index_context.rs → Index input types
//! ├── types.rs         → Public API types
//! ├── context.rs       → Request context and configuration
//! ├── session.rs       → Session management
//! ├── indexer.rs       → Document indexing operations
//! ├── retriever.rs     → Query and retrieval operations
//! ├── workspace.rs     → Workspace CRUD operations
//! └── events.rs        → Event system and callbacks
//! ```
//!
//! # Quick Start
//!
//! ```rust,no_run
//! use vectorless::client::{Engine, EngineBuilder, IndexContext};
//!
//! # #[tokio::main]
//! # async fn main() -> vectorless::domain::Result<()> {
//! // Create a client with default settings
//! let client = EngineBuilder::new()
//!     .with_workspace("./my_workspace")
//!     .build()?;
//!
//! // Index a document from file
//! let doc_id = client.index(IndexContext::from_path("./document.md")).await?;
//!
//! // Index HTML content directly
//! let html = "<html><body><h1>Title</h1><p>Content</p></body></html>";
//! let doc_id2 = client.index(
//!     IndexContext::from_content(html, vectorless::parser::DocumentFormat::Html)
//!         .with_name("webpage")
//! ).await?;
//!
//! // Query the document
//! let result = client.query(&doc_id, "What is this?").await?;
//! println!("{}", result.content);
//!
//! // List all documents
//! for doc in client.list_documents().await? {
//!     println!("{}: {}", doc.id, doc.name);
//! }
//! # Ok(())
//! # }
//! ```
//!
//! # Session-Based Operations
//!
//! For multi-document operations, use sessions:
//!
//! ```rust,no_run
//! # use vectorless::client::{Engine, EngineBuilder, IndexContext};
//! # #[tokio::main]
//! # async fn main() -> vectorless::domain::Result<()> {
//! let client = EngineBuilder::new()
//!     .with_workspace("./workspace")
//!     .build()?;
//!
//! let session = client.session();
//!
//! // Index multiple documents
//! let doc1 = session.index(IndexContext::from_path("./doc1.md")).await?;
//! let doc2 = session.index(IndexContext::from_path("./doc2.md")).await?;
//!
//! // Query across all documents
//! let results = session.query_all("What is the architecture?").await?;
//! # Ok(())
//! # }
//! ```
//!
//! # Events and Progress
//!
//! Monitor operation progress with events:
//!
//! ```rust,no_run
//! # use vectorless::client::{Engine, EngineBuilder, EventEmitter, events::IndexEvent};
//! # #[tokio::main]
//! # async fn main() -> vectorless::domain::Result<()> {
//! let events = EventEmitter::new()
//!     .on_index(|e| match e {
//!         IndexEvent::Complete { doc_id } => println!("Indexed: {}", doc_id),
//!         _ => {}
//!     });
//!
//! let client = EngineBuilder::new()
//!     .with_events(events)
//!     .build()?;
//! # Ok(())
//! # }
//! ```
//!
//! # Features
//!
//! - **Document Indexing** — Parse and index Markdown, PDF, and text files
//! - **Tree-Based Structure** — Documents organized as hierarchical trees
//! - **Workspace Persistence** — Save and load indexed documents
//! - **Session Management** — Multi-document operations with caching
//! - **Event System** — Progress callbacks and monitoring

mod builder;
mod context;
mod engine;
pub mod events;
mod index_context;
mod indexer;
mod retriever;
mod session;
mod types;
mod workspace;

// ============================================================
// Main Types
// ============================================================

pub use builder::{BuildError, EngineBuilder};
pub use engine::Engine;

// ============================================================
// Index Context
// ============================================================

pub use index_context::{IndexContext, IndexSource};

// ============================================================
// Sub-Clients
// ============================================================

pub use indexer::IndexerClient;
pub use retriever::RetrieverClient;
pub use session::Session;
pub use workspace::WorkspaceClient;

// ============================================================
// Context and Events
// ============================================================

pub use context::{ClientContext, FeatureFlags, RequestContextConfig};
pub use events::{
    AsyncEventHandler, Event, EventEmitter, EventHandler, IndexEvent, QueryEvent, WorkspaceEvent,
};

// ============================================================
// Types
// ============================================================

pub use types::{
    // Error types
    ClientError,
    // Document info
    DocumentInfo,
    // Index types
    IndexMode,
    IndexOptions,
    // Document types
    IndexedDocument,
    PageContent,
    // Query types
    QueryResult,
};

// ============================================================
// Sub-Client Types
// ============================================================

pub use indexer::{IndexerConfig, ValidationResult};
pub use retriever::{NodeContext, RetrieverClientConfig};
pub use session::{EvictionPolicy, PreloadStrategy, SessionConfig, SessionStats};
pub use workspace::{WorkspaceClientConfig, WorkspaceStats};
