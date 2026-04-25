// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Event system for observing and reacting to client operations.
//!
//! This module provides event types and the [`EventEmitter`] for
//! registering handlers and dispatching events during compilation,
//! querying, and workspace operations.
//!
//! # Example
//!
//! ```rust,ignore
//! use vectorless::events::{EventEmitter, CompileEvent};
//!
//! let emitter = EventEmitter::new()
//!     .on_compile(|e| match e {
//!         CompileEvent::Complete { doc_id } => println!("Compiled: {}", doc_id),
//!         _ => {}
//!     });
//!
//! let client = EngineBuilder::new()
//!     .with_events(emitter)
//!     .build()
//!     .await?;
//! ```

mod emitter;
mod types;

pub use emitter::EventEmitter;
pub use types::{CompileEvent, WorkspaceEvent};
