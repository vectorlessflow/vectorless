// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Event system for client operations.
//!
//! This module provides event types and handlers for observing
//! and reacting to client operations (indexing, querying, etc.).
//!
//! # Example
//!
//! ```rust,ignore
//! let emitter = EventEmitter::new()
//!     .on_index(|e| match e {
//!         IndexEvent::Complete { doc_id } => println!("Indexed: {}", doc_id),
//!         _ => {}
//!     });
//!
//! let client = EngineBuilder::new()
//!     .with_events(emitter)
//!     .build()?;
//! ```

use std::sync::Arc;

use async_trait::async_trait;
use tracing::info;

use crate::parser::DocumentFormat;
use crate::retrieval::SufficiencyLevel;

/// Event types for client operations.
#[derive(Debug, Clone)]
pub enum Event {
    /// Indexing events.
    Index(IndexEvent),

    /// Query events.
    Query(QueryEvent),

    /// Workspace events.
    Workspace(WorkspaceEvent),
}

/// Indexing operation events.
#[derive(Debug, Clone)]
pub enum IndexEvent {
    /// Started indexing a document.
    Started {
        /// File path being indexed.
        path: String,
    },

    /// Document format detected.
    FormatDetected {
        /// Detected format.
        format: DocumentFormat,
    },

    /// Parsing progress update.
    ParsingProgress {
        /// Percentage complete (0-100).
        percent: u8,
    },

    /// Document tree built.
    TreeBuilt {
        /// Number of nodes in the tree.
        node_count: usize,
    },

    /// Summary generation progress.
    SummaryProgress {
        /// Number of summaries completed.
        completed: usize,
        /// Total summaries to generate.
        total: usize,
    },

    /// Indexing completed successfully.
    Complete {
        /// Generated document ID.
        doc_id: String,
    },

    /// Error occurred during indexing.
    Error {
        /// Error message.
        message: String,
    },
}

/// Query operation events.
#[derive(Debug, Clone)]
pub enum QueryEvent {
    /// Search started.
    Started {
        /// The query string.
        query: String,
    },

    /// Node visited during search.
    NodeVisited {
        /// Node ID.
        node_id: String,
        /// Node title.
        title: String,
        /// Relevance score.
        score: f32,
    },

    /// Candidate result found.
    CandidateFound {
        /// Node ID.
        node_id: String,
        /// Relevance score.
        score: f32,
    },

    /// Sufficiency check result.
    SufficiencyCheck {
        /// Sufficiency level.
        level: SufficiencyLevel,
        /// Total tokens collected.
        tokens: usize,
    },

    /// Query completed.
    Complete {
        /// Total results found.
        total_results: usize,
        /// Overall confidence score.
        confidence: f32,
    },

    /// Error occurred during query.
    Error {
        /// Error message.
        message: String,
    },
}

/// Workspace operation events.
#[derive(Debug, Clone)]
pub enum WorkspaceEvent {
    /// Document saved to workspace.
    Saved {
        /// Document ID.
        doc_id: String,
    },

    /// Document loaded from workspace.
    Loaded {
        /// Document ID.
        doc_id: String,
        /// Whether it was a cache hit.
        cache_hit: bool,
    },

    /// Document removed from workspace.
    Removed {
        /// Document ID.
        doc_id: String,
    },

    /// Workspace cleared.
    Cleared {
        /// Number of documents removed.
        count: usize,
    },
}

/// Sync event handler trait.
pub(crate) trait EventHandler: Send + Sync {
    /// Handle an event.
    fn handle(&self, event: &Event);
}

/// Async event handler trait.
#[async_trait]
pub(crate) trait AsyncEventHandler: Send + Sync {
    /// Handle an event asynchronously.
    async fn handle(&self, event: &Event);
}

/// Type alias for sync index handler.
pub(crate) type IndexHandler = Box<dyn Fn(&IndexEvent) + Send + Sync>;

/// Type alias for sync query handler.
pub(crate) type QueryHandler = Box<dyn Fn(&QueryEvent) + Send + Sync>;

/// Type alias for sync workspace handler.
pub(crate) type WorkspaceHandler = Box<dyn Fn(&WorkspaceEvent) + Send + Sync>;

/// Event emitter for client operations.
///
/// Collects event handlers and dispatches events to them.
#[derive(Default)]
pub struct EventEmitter {
    /// Index event handlers.
    index_handlers: Vec<IndexHandler>,

    /// Query event handlers.
    query_handlers: Vec<QueryHandler>,

    /// Workspace event handlers.
    workspace_handlers: Vec<WorkspaceHandler>,

    /// Async handlers.
    async_handlers: Vec<Arc<dyn AsyncEventHandler>>,
}

impl EventEmitter {
    /// Create a new event emitter with no handlers.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add an index event handler.
    pub fn on_index<F>(mut self, handler: F) -> Self
    where
        F: Fn(&IndexEvent) + Send + Sync + 'static,
    {
        self.index_handlers.push(Box::new(handler));
        self
    }

    /// Add a query event handler.
    pub fn on_query<F>(mut self, handler: F) -> Self
    where
        F: Fn(&QueryEvent) + Send + Sync + 'static,
    {
        self.query_handlers.push(Box::new(handler));
        self
    }

    /// Add a workspace event handler.
    pub fn on_workspace<F>(mut self, handler: F) -> Self
    where
        F: Fn(&WorkspaceEvent) + Send + Sync + 'static,
    {
        self.workspace_handlers.push(Box::new(handler));
        self
    }

    /// Add an async event handler.
    pub(crate) fn with_async_handler<H>(mut self, handler: Arc<H>) -> Self
    where
        H: AsyncEventHandler + 'static,
    {
        self.async_handlers.push(handler);
        self
    }

    /// Emit an index event.
    pub fn emit_index(&self, event: IndexEvent) {
        for handler in &self.index_handlers {
            handler(&event);
        }
        for handler in &self.async_handlers {
            // For sync context, we just log async handlers
            let event = Event::Index(event.clone());
            info!("Async event: {:?}", event);
        }
    }

    /// Emit a query event.
    pub fn emit_query(&self, event: QueryEvent) {
        for handler in &self.query_handlers {
            handler(&event);
        }
    }

    /// Emit a workspace event.
    pub fn emit_workspace(&self, event: WorkspaceEvent) {
        for handler in &self.workspace_handlers {
            handler(&event);
        }
    }

    /// Check if there are any handlers registered.
    pub fn has_handlers(&self) -> bool {
        !self.index_handlers.is_empty()
            || !self.query_handlers.is_empty()
            || !self.workspace_handlers.is_empty()
            || !self.async_handlers.is_empty()
    }

    /// Merge another emitter into this one.
    pub fn merge(mut self, other: EventEmitter) -> Self {
        self.index_handlers.extend(other.index_handlers);
        self.query_handlers.extend(other.query_handlers);
        self.workspace_handlers.extend(other.workspace_handlers);
        self.async_handlers.extend(other.async_handlers);
        self
    }
}

impl std::fmt::Debug for EventEmitter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EventEmitter")
            .field("index_handlers", &self.index_handlers.len())
            .field("query_handlers", &self.query_handlers.len())
            .field("workspace_handlers", &self.workspace_handlers.len())
            .field("async_handlers", &self.async_handlers.len())
            .finish()
    }
}

impl Clone for EventEmitter {
    fn clone(&self) -> Self {
        // Clone returns an empty emitter since we can't clone closures
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn test_event_emitter_index() {
        let counter = Arc::new(AtomicUsize::new(0));
        let counter_clone = counter.clone();

        let emitter = EventEmitter::new().on_index(move |_e| {
            counter_clone.fetch_add(1, Ordering::SeqCst);
        });

        emitter.emit_index(IndexEvent::Started {
            path: "test.md".to_string(),
        });
        emitter.emit_index(IndexEvent::Complete {
            doc_id: "123".to_string(),
        });

        assert_eq!(counter.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn test_event_emitter_query() {
        let counter = Arc::new(AtomicUsize::new(0));
        let counter_clone = counter.clone();

        let emitter = EventEmitter::new().on_query(move |_e| {
            counter_clone.fetch_add(1, Ordering::SeqCst);
        });

        emitter.emit_query(QueryEvent::Started {
            query: "test".to_string(),
        });

        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn test_event_emitter_has_handlers() {
        let empty = EventEmitter::new();
        assert!(!empty.has_handlers());

        let with_handler = EventEmitter::new().on_index(|_| {});
        assert!(with_handler.has_handlers());
    }
}
