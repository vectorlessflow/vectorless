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
pub mod events;
pub mod state;
pub mod tools;

// Sub-modules for loop implementations:
pub mod orchestrator;
pub mod prompts;
pub mod subagent;

pub use config::{Config, DocContext, Output, QueryComplexity, Scope, WorkspaceContext};
pub use events::{AgentEvent, EventEmitter};

/// Retrieve information from documents using the agent.
///
/// This is the single public entry point for all retrieval operations.
/// Based on the [`Scope`], it routes to either:
/// - Direct SubAgent (single document)
/// - Orchestrator + SubAgents (workspace/multi-doc)
pub async fn retrieve(
    query: &str,
    scope: Scope<'_>,
    config: &Config,
    llm: &crate::llm::LlmClient,
    emitter: &EventEmitter,
) -> crate::error::Result<Output> {
    match scope {
        Scope::Single(doc_ctx) => {
            // User specified a document → SubAgent directly
            subagent::run(query, None, &doc_ctx, config, llm, emitter).await
        }
        Scope::Workspace(ws_ctx) => {
            // Multi-doc / workspace → Orchestrator
            orchestrator::run(query, &ws_ctx, config, llm, emitter).await
        }
    }
}
