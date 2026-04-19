// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Retrieval agent — pure-function document intelligence.
//!
//! # Architecture
//!
//! The retrieval dispatcher always goes through the Orchestrator.
//! Based on [`Scope`]:
//!
//! - **User specified doc_ids** → Orchestrator skips analysis, spawns SubAgents directly.
//! - **Workspace / unspecified** → Orchestrator analyzes DocCards, selects docs, spawns SubAgents.
//!
//! Both paths produce the same [`Output`] type and share the same synthesis logic.
//!
//! ```text
//! dispatch(query, scope)
//!     └── Orchestrator (always)
//!          ├── Scope::Specified(docs) → skip analysis → N × SubAgent → synthesis
//!          └── Scope::Workspace(ws)  → analysis → N × SubAgent → fusion → synthesis
//! ```

pub mod command;
pub mod config;
pub mod context;
pub mod events;
pub mod state;
pub mod tools;

// Sub-modules for loop implementations:
pub mod orchestrator;
pub mod prompts;
pub mod subagent;

pub use config::{Config, DocContext, Evidence, Metrics, Output, Scope, WorkspaceContext};
pub use events::{AgentEvent, EventEmitter};
