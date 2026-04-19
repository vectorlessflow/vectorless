// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Retrieval infrastructure — types, streaming, and caching.
//!
//! The actual retrieval engine lives in the top-level [`agent`](crate::agent) module.
//! This module provides supporting infrastructure:
//!
//! - **Types** — `RetrieveResponse`, `SufficiencyLevel`, `ReasoningChain`, etc.
//! - **Streaming** — `RetrieveEvent` / `RetrieveEventReceiver` for async progress
//! - **Cache** — `ReasoningCache` for L1 query caching

mod cache;
pub mod stream;
mod types;

pub use stream::RetrieveEventReceiver;
pub use types::*;
