// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! High-level client API for document indexing and retrieval.
//!
//! This module provides the main entry point for using vectorless:
//! - [`Engine`] — The main client for indexing and querying documents
//! - [`EngineBuilder`] — Builder pattern for client configuration
//! - [`Session`] — Multi-document session management
//!
//! # Architecture
//!
//! The client module is organized into specialized sub-modules:
//!
//! ```text
//! client/
//! ├── mod.rs          → Re-exports and documentation
//! ├── engine.rs       → Main orchestrator
//! ├── builder.rs      → Builder pattern
//! ├── types.rs        → Public API types
//! ├── context.rs      → Request context and configuration
//! ├── session.rs      → Session management
//! ├── indexer.rs      → Document indexing operations
//! ├── retriever.rs    → Query and retrieval operations
//! ├── workspace.rs    → Workspace CRUD operations
//! └── events.rs       → Event system and callbacks
//! ```
//!
//! # Quick Start
//!
//! ```rust,no_run
//! use vectorless::client::{Engine, EngineBuilder};
//!
//! # #[tokio::main]
//! # async fn main() -> vectorless::domain::Result<()> {
//! // Create a client with default settings
//! let client = EngineBuilder::new()
//!     .with_workspace("./my_workspace")
//!     .build()?;
//!
//! // Index a document
//! let doc_id = client.index("./document.md").await?;
//!
//! // Get document structure
//! let structure = client.get_structure(&doc_id)?;
//!
//! // Query the document
//! let result = client.query(&doc_id, "What is this?").await?;
//! println!("{}", result.content);
//!
//! // List all documents
//! for doc in client.list_documents() {
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
//! # use vectorless::client::{Engine, EngineBuilder};
//! # #[tokio::main]
//! # async fn main() -> vectorless::domain::Result<()> {
//! let client = EngineBuilder::new()
//!     .with_workspace("./workspace")
//!     .build()?;
//!
//! let session = client.session();
//!
//! // Index multiple documents
//! let doc1 = session.index("./doc1.md").await?;
//! let doc2 = session.index("./doc2.md").await?;
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
mod events;
mod indexer;
mod retriever;
mod session;
mod types;
mod workspace;

// ============================================================
// Main Types
// ============================================================

pub use engine::Engine;
pub use builder::{BuildError, EngineBuilder};

// ============================================================
// Sub-Clients
// ============================================================

pub use indexer::IndexerClient;
pub use retriever::RetrieverClient;
pub use workspace::WorkspaceClient;
pub use session::Session;

// ============================================================
// Context and Events
// ============================================================

pub use context::{ClientContext, FeatureFlags, RequestContextConfig};
pub use events::{
    EventEmitter, Event, EventHandler, AsyncEventHandler,
    IndexEvent, QueryEvent, WorkspaceEvent,
};

// ============================================================
// Types
// ============================================================

pub use types::{
    // Document types
    IndexedDocument, PageContent,
    // Index types
    IndexMode, IndexOptions,
    // Query types
    QueryResult,
    // Document info
    DocumentInfo,
    // Error types
    ClientError,
};

// ============================================================
// Sub-Client Types
// ============================================================

pub use indexer::{IndexerConfig, ValidationResult};
pub use retriever::{RetrieverClientConfig, NodeContext};
pub use workspace::{WorkspaceClientConfig, WorkspaceStats};
pub use session::{SessionConfig, SessionStats, EvictionPolicy, PreloadStrategy};
