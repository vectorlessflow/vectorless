// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Configuration management for vectorless.
//!
//! This module provides configuration loading and validation:
//! - [`Config`] - Main configuration structure
//! - [`IndexerConfig`] - Indexing parameters
//! - [`SummaryConfig`] - Summarization model settings
//! - [`RetrievalConfig`] - Retrieval model settings
//! - [`StorageConfig`] - Storage paths

mod types;
mod loader;

pub use types::*;
pub use loader::{ConfigLoader, ConfigError};
