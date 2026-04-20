// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Tool definitions for the retrieval agent.
//!
//! Tools are organized by role:
//! - `common` — shared between Orchestrator and Worker (find, check, done)
//! - `worker` — Worker-specific (ls, cd, cd_up, cat, pwd)
//! - `orchestrator` — Orchestrator-specific (ls_docs, find_cross, dispatch)

pub mod common;
pub mod orchestrator;
pub mod worker;

/// Result of executing a tool command.
#[derive(Debug, Clone)]
pub struct ToolResult {
    /// Text feedback to include in the next LLM prompt.
    pub feedback: String,
    /// Whether the loop should stop.
    pub should_stop: bool,
    /// Whether the command executed successfully.
    pub success: bool,
}

impl ToolResult {
    /// Create a successful result with feedback.
    pub fn ok(feedback: impl Into<String>) -> Self {
        Self {
            feedback: feedback.into(),
            should_stop: false,
            success: true,
        }
    }

    /// Create a result that signals loop termination.
    pub fn done(feedback: impl Into<String>) -> Self {
        Self {
            feedback: feedback.into(),
            should_stop: true,
            success: true,
        }
    }

    /// Create a failed result (parse error, invalid target, etc.).
    pub fn fail(feedback: impl Into<String>) -> Self {
        Self {
            feedback: feedback.into(),
            should_stop: false,
            success: false,
        }
    }
}
