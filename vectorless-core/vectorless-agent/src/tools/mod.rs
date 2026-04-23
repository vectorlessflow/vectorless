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

/// Extract a content snippet around the first occurrence of `keyword`.
///
/// Returns `None` if the content is empty. If the keyword is not found,
/// returns the beginning of the content instead.
pub fn content_snippet(content: &str, keyword: &str, max_len: usize) -> Option<String> {
    if content.trim().is_empty() {
        return None;
    }

    let keyword_lower = keyword.to_lowercase();
    let content_lower = content.to_lowercase();

    let start = match content_lower.find(&keyword_lower) {
        Some(pos) => {
            let back = (max_len / 4).min(pos);
            pos - back
        }
        None => 0,
    };

    let start = content
        .char_indices()
        .find(|(i, _)| *i >= start)
        .map(|(i, _)| i)
        .unwrap_or(0);

    let end = content
        .char_indices()
        .take_while(|(i, _)| *i <= start + max_len)
        .last()
        .map(|(i, c)| i + c.len_utf8())
        .unwrap_or(content.len());

    let snippet = content[start..end].trim();
    if snippet.is_empty() {
        return None;
    }

    let mut result = snippet.to_string();
    if end < content.len() {
        result.push_str("...");
    }
    if start > 0 {
        result = format!("...{}", result);
    }
    Some(result)
}
