// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Retrieval agent — struct-based document intelligence.
//!
//! # Architecture
//!
//! The retrieval dispatcher always goes through the Orchestrator.
//! Based on [`Scope`]:
//!
//! - **User specified doc_ids** → Orchestrator skips analysis, spawns Workers directly.
//! - **Workspace / unspecified** → Orchestrator analyzes DocCards, selects docs, spawns Workers.
//!
//! Both paths produce the same [`Output`] type and share the same synthesis logic.
//!
//! ```text
//! dispatch(query, scope)
//!     └── Orchestrator (always)
//!          ├── Scope::Specified(docs) → skip analysis → N × Worker → synthesis
//!          └── Scope::Workspace(ws)  → analysis → N × Worker → fusion → synthesis
//! ```
//!
//! # Agent trait
//!
//! All retrieval agents implement [`Agent`] with `async fn run(self)` (Edition 2024).
//! The trait uses native async functions — no `async-trait` crate needed.

pub mod command;
pub mod config;
pub mod context;
pub mod events;
pub mod state;
pub mod tools;

pub mod orchestrator;
pub mod prompts;
pub mod worker;

pub use config::{DocContext, Evidence, Output, Scope, WorkspaceContext};
pub use events::{AgentEvent, EventEmitter};

/// Agent trait — async, consuming-self execution.
///
/// Each agent struct holds its own configuration and context.
/// Calling `run(self)` consumes the agent and produces output.
///
/// Uses Edition 2024 native `async fn` in trait — no `async-trait` crate.
pub trait Agent {
    /// The output type produced by this agent.
    type Output;
    /// Agent name for logging and events.
    fn name(&self) -> &str;
    /// Execute the agent, consuming self.
    async fn run(self) -> vectorless_error::Result<Self::Output>;
}
