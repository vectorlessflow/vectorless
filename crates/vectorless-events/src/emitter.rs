// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Event emitter for client operations.
//!
//! Collects event handlers and dispatches events to them.
//! Uses `Arc<RwLock<Inner>>` so cloning shares handlers instead of losing them.

use std::sync::Arc;

use parking_lot::RwLock;

use super::types::{IndexEvent, WorkspaceEvent};

/// Type alias for sync index handler.
pub(crate) type IndexHandler = Box<dyn Fn(&IndexEvent) + Send + Sync>;

/// Type alias for sync workspace handler.
pub(crate) type WorkspaceHandler = Box<dyn Fn(&WorkspaceEvent) + Send + Sync>;

/// Inner state shared via `Arc<RwLock<...>>`.
struct EventEmitterInner {
    /// Index event handlers.
    index_handlers: Vec<IndexHandler>,

    /// Workspace event handlers.
    workspace_handlers: Vec<WorkspaceHandler>,
}

impl Default for EventEmitterInner {
    fn default() -> Self {
        Self {
            index_handlers: Vec::new(),
            workspace_handlers: Vec::new(),
        }
    }
}

/// Event emitter for client operations.
///
/// Collects event handlers and dispatches events to them.
/// Cloning shares the same handlers (via `Arc`), so all clones
/// dispatch to the same registered handlers.
///
/// # Example
///
/// ```rust,ignore
/// let emitter = EventEmitter::new()
///     .on_index(|e| match e {
///         IndexEvent::Complete { doc_id } => println!("Indexed: {}", doc_id),
///         _ => {}
///     });
///
/// let clone = emitter.clone();
/// // clone shares the same handlers — emitting on either fires on both
/// ```
pub struct EventEmitter {
    inner: Arc<RwLock<EventEmitterInner>>,
}

impl EventEmitter {
    /// Create a new event emitter with no handlers.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add an index event handler.
    pub fn on_index<F>(self, handler: F) -> Self
    where
        F: Fn(&IndexEvent) + Send + Sync + 'static,
    {
        self.inner.write().index_handlers.push(Box::new(handler));
        self
    }

    /// Add a workspace event handler.
    pub fn on_workspace<F>(self, handler: F) -> Self
    where
        F: Fn(&WorkspaceEvent) + Send + Sync + 'static,
    {
        self.inner
            .write()
            .workspace_handlers
            .push(Box::new(handler));
        self
    }

    /// Emit an index event.
    pub fn emit_index(&self, event: IndexEvent) {
        let inner = self.inner.read();
        for handler in &inner.index_handlers {
            handler(&event);
        }
    }

    /// Emit a workspace event.
    pub fn emit_workspace(&self, event: WorkspaceEvent) {
        let inner = self.inner.read();
        for handler in &inner.workspace_handlers {
            handler(&event);
        }
    }

    /// Check if there are any handlers registered.
    pub fn has_handlers(&self) -> bool {
        let inner = self.inner.read();
        !inner.index_handlers.is_empty() || !inner.workspace_handlers.is_empty()
    }

    /// Merge another emitter into this one.
    pub fn merge(self, other: EventEmitter) -> Self {
        let mut other_inner = other.inner.write();
        let mut inner = self.inner.write();
        inner
            .index_handlers
            .extend(other_inner.index_handlers.drain(..));
        inner
            .workspace_handlers
            .extend(other_inner.workspace_handlers.drain(..));
        drop(inner);
        drop(other_inner);
        self
    }
}

impl Default for EventEmitter {
    fn default() -> Self {
        Self {
            inner: Arc::new(RwLock::new(EventEmitterInner::default())),
        }
    }
}

impl Clone for EventEmitter {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}

impl std::fmt::Debug for EventEmitter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let inner = self.inner.read();
        f.debug_struct("EventEmitter")
            .field("index_handlers", &inner.index_handlers.len())
            .field("workspace_handlers", &inner.workspace_handlers.len())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
    fn test_event_emitter_has_handlers() {
        let empty = EventEmitter::new();
        assert!(!empty.has_handlers());

        let with_handler = EventEmitter::new().on_index(|_| {});
        assert!(with_handler.has_handlers());
    }

    #[test]
    fn test_event_emitter_clone_shares_handlers() {
        let counter = Arc::new(AtomicUsize::new(0));
        let counter_clone = counter.clone();

        let emitter = EventEmitter::new().on_index(move |_e| {
            counter_clone.fetch_add(1, Ordering::SeqCst);
        });

        let cloned = emitter.clone();

        // Emit on the clone — original's handler should fire
        cloned.emit_index(IndexEvent::Started {
            path: "test.md".to_string(),
        });

        assert_eq!(counter.load(Ordering::SeqCst), 1);

        // Emit on the original too
        emitter.emit_index(IndexEvent::Complete {
            doc_id: "123".to_string(),
        });

        assert_eq!(counter.load(Ordering::SeqCst), 2);
    }
}
