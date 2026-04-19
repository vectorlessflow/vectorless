// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Retrieval dispatch layer — the entry point for all query operations.
//!
//! This module sits between the client API and the agent execution layer.
//! It is responsible for:
//!
//! - **Dispatching** queries to the appropriate agent path (SubAgent vs Orchestrator)
//! - **Post-processing** agent output into client-facing results
//! - **Caching** query results (L1 exact, L2 path patterns, L3 strategy scores)
//! - **Streaming** retrieval events for async progress monitoring
//!
//! Call flow:
//! ```text
//! client → retrieval::dispatch()
//!   ├── User specified doc_ids → parallel N × SubAgent
//!   └── Workspace scope → Orchestrator (analyze → spawn → fusion)
//! ```

mod cache;
pub mod dispatcher;
pub mod postprocessor;
pub mod stream;
mod types;

pub use stream::RetrieveEventReceiver;
pub use types::*;
