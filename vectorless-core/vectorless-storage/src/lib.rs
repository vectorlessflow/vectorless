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

pub mod backend;
pub mod cache;
pub mod codec;
pub mod lock;
pub mod migration;
mod persistence;
pub mod workspace;

// Re-export main types
pub use persistence::{DocumentMeta, PageContent, PersistedDocument};
pub use workspace::Workspace;
