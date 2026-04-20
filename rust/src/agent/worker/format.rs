// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Formatting helpers for Worker prompts.

use super::super::config::DocContext;
use super::super::state::WorkerState;

/// Resolve visited NodeIds to their titles for prompt injection.
pub fn format_visited_titles(state: &WorkerState, ctx: &DocContext<'_>) -> String {
    if state.visited.is_empty() {
        return "(none)".to_string();
    }
    state
        .visited
        .iter()
        .filter_map(|&node_id| ctx.node_title(node_id).map(|t| t.to_string()))
        .collect::<Vec<_>>()
        .join(", ")
}
