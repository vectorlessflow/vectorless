// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Caching for retrieval operations.
//!
//! Caches search paths and node scores for repeated queries.

mod path_cache;

pub use path_cache::PathCache;
