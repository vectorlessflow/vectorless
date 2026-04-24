// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Storage backend abstraction.
//!
//! This module provides a trait-based abstraction for different storage backends,
//! allowing the workspace to work with various storage systems:
//!
//! - **FileBackend**: File system storage (default)
//! - **MemoryBackend**: In-memory storage (for testing)
//!
//! # Example
//!
//! ```rust,ignore
//! use vectorless::storage::backend::{StorageBackend, FileBackend};
//!
//! let backend = FileBackend::new("./workspace");
//!
//! // Store data
//! backend.put("doc-1", b"document data")?;
//!
//! // Retrieve data
//! let data = backend.get("doc-1")?;
//!
//! // List all keys
//! let keys = backend.keys()?;
//! ```

mod file;
mod trait_def;

pub use file::FileBackend;
pub use trait_def::StorageBackend;
