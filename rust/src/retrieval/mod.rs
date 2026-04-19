// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Retrieval system for Vectorless document trees.
//!
//! This module implements agent-based retrieval:
//! - **SubAgent**: navigates a single document (ls → cd → cat → check → done)
//! - **Orchestrator**: multi-document MapReduce (analyze → dispatch → integrate → synthesize)
//!
//! # Architecture
//!
//! ```text
//! retrieve(query, scope)
//!     ├── Scope::Single(doc)     → SubAgent loop → Output
//!     └── Scope::Workspace(ws)   → Orchestrator → Output
//! ```

pub mod stream;
mod types;

pub mod agent;
pub mod cache;
pub mod complexity;
pub mod scoring;
pub mod sufficiency;

pub use types::*;
pub use stream::RetrieveEventReceiver;
