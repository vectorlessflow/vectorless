// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Event emitter for client operations.
//!
//! Collects event handlers and dispatches events to them.
//! Uses `Arc<RwLock<Inner>>` so cloning shares handlers instead of losing them.

use std::sync::Arc;

use parking_lot::RwLock;

use super::types::{CompileEvent, WorkspaceEvent};

/// Type alias for sync compile handler.
pub(crate) type CompileHandler = Box<dyn Fn(&CompileEvent) + Send + Sync>;

/// Type alias for sync workspace handler.
pub(crate) type WorkspaceHandler = Box<dyn Fn(&WorkspaceEvent) + Send + Sync>;

/// Inner state shared via `Arc<RwLock<...>>`.
struct EventEmitterInner {
    /// Compile event handlers.
    compile_handlers: Vec<CompileHandler>,

    /// Workspace event handlers.
    workspace_handlers: Vec<WorkspaceHandler>,
}

impl Default for EventEmitterInner {
    fn default() -> Self {
        Self {
            compile_handlers: Vec::new(),
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
///     .on_compile(|e| match e {
///         CompileEvent::Complete { doc_id } => println!("Compiled: {}", doc_id),
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

    /// Add a compile event handler.
    pub fn on_compile<F>(self, handler: F) -> Self
    where
        F: Fn(&CompileEvent) + Send + Sync + 'static,
    {
        self.inner.write().compile_handlers.push(Box::new(handler));
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

    /// Emit a compile event.
    pub fn emit_compile(&self, event: CompileEvent) {
        let inner = self.inner.read();
        for handler in &inner.compile_handlers {
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
        !inner.compile_handlers.is_empty() || !inner.workspace_handlers.is_empty()
    }

    /// Merge another emitter into this one.
    pub fn merge(self, other: EventEmitter) -> Self {
        let mut other_inner = other.inner.write();
        let mut inner = self.inner.write();
        inner
            .compile_handlers
            .extend(other_inner.compile_handlers.drain(..));
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
            .field("compile_handlers", &inner.compile_handlers.len())
            .field("workspace_handlers", &inner.workspace_handlers.len())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn test_event_emitter_compile() {
        let counter = Arc::new(AtomicUsize::new(0));
        let counter_clone = counter.clone();

        let emitter = EventEmitter::new().on_compile(move |_e| {
            counter_clone.fetch_add(1, Ordering::SeqCst);
        });

        emitter.emit_compile(CompileEvent::Started {
            path: "test.md".to_string(),
        });
        emitter.emit_compile(CompileEvent::Complete {
            doc_id: "123".to_string(),
        });

        assert_eq!(counter.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn test_event_emitter_has_handlers() {
        let empty = EventEmitter::new();
        assert!(!empty.has_handlers());

        let with_handler = EventEmitter::new().on_compile(|_| {});
        assert!(with_handler.has_handlers());
    }

    #[test]
    fn test_event_emitter_clone_shares_handlers() {
        let counter = Arc::new(AtomicUsize::new(0));
        let counter_clone = counter.clone();

        let emitter = EventEmitter::new().on_compile(move |_e| {
            counter_clone.fetch_add(1, Ordering::SeqCst);
        });

        let cloned = emitter.clone();

        // Emit on the clone — original's handler should fire
        cloned.emit_compile(CompileEvent::Started {
            path: "test.md".to_string(),
        });

        assert_eq!(counter.load(Ordering::SeqCst), 1);

        // Emit on the original too
        emitter.emit_compile(CompileEvent::Complete {
            doc_id: "123".to_string(),
        });

        assert_eq!(counter.load(Ordering::SeqCst), 2);
    }
}
