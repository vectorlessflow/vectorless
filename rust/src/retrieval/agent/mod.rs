// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Retrieval agent — pure-function document intelligence.
//!
//! # Architecture
//!
//! Single entry point: [`retrieve()`]. Routes based on scope:
//!
//! - **User specifies doc_id** → SubAgent runs directly on that document.
//! - **Workspace / multi-doc / unspecified** → Orchestrator analyzes all DocCards,
//!   dispatches N SubAgents in parallel, integrates results.
//!
//! Both paths produce the same [`Output`] type.
//!
//! ```text
//! retrieve(query, context)
//!     ├── RetrievalContext::Single(doc)    → SubAgent loop → Output
//!     └── RetrievalContext::Workspace(ws)  → Orchestrator → Output
//! ```

pub mod command;
pub mod config;
pub mod context;
pub mod state;
pub mod tools;

// Sub-modules for loop implementations (Phase 3/4):
// pub mod subagent;
// pub mod orchestrator;
pub mod prompts;

pub use command::Command;
pub use config::{
    Config, DocContext, Evidence, Metrics, Output, Scope, Step, WorkspaceContext,
};
pub use context::FindHit;
pub use prompts::{DispatchEntry, parse_dispatch_plan, parse_sufficiency_response};
pub use state::{OrchestratorState, State};

/// Retrieve information from documents using the agent.
///
/// This is the single public entry point for all retrieval operations.
/// Based on the [`Scope`], it routes to either:
/// - Direct SubAgent (single document)
/// - Orchestrator + SubAgents (workspace/multi-doc)
///
/// Currently returns a placeholder. Full implementation in Phase 3/4.
pub async fn retrieve(
    _query: &str,
    _scope: Scope<'_>,
    _config: &Config,
) -> crate::error::Result<Output> {
    // Phase 3/4: wire up subagent and orchestrator loops
    todo!("agent retrieve — implement in Phase 3/4")
}
