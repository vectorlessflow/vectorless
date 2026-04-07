// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! LLM Memoization system for caching expensive LLM calls.
//!
//! This module provides a caching layer for LLM-generated content,
//! enabling significant cost savings by avoiding redundant API calls.
//!
//! # Key Features
//!
//! - **Operation-based caching**: Cache summaries, pilot decisions, query results
//! - **Content-addressed**: Keys are based on content fingerprints
//! - **TTL support**: Optional time-to-live for cache entries
//! - **Persistence**: Save/load cache to disk for cross-session reuse
//!
//! # Usage
//!
//! ```rust,ignore
//! use vectorless::memo::{MemoStore, MemoKey, MemoOpType};
//!
//! // Create a memo store
//! let mut store = MemoStore::new(1000);
//!
//! // Get or compute a summary
//! let key = MemoKey::summary(&node_fingerprint);
//! let summary = store.get_or_compute(key, || async {
//!     llm_client.generate_summary(node).await
//! }).await?;
//! ```

mod store;
mod types;

pub use store::MemoStore;
pub use types::{MemoEntry, MemoKey, MemoOpType, MemoStats, MemoValue, PilotDecisionValue};
