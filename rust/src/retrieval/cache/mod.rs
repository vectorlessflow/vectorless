// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Caching for retrieval operations.
//!
//! Three-tier reasoning cache:
//! - **L1**: Exact query match — instant cache hit for repeated queries
//! - **L2**: Path pattern cache — reuse navigation decisions across queries
//! - **L3**: Strategy score cache — share keyword/BM25 scores across queries
//!
//! Legacy `PathCache` remains for backward compatibility.

mod hot_tracker;
mod path_cache;
mod reasoning_cache;

pub use hot_tracker::HotNodeTracker;
pub use reasoning_cache::{CachedCandidate, ReasoningCache};
