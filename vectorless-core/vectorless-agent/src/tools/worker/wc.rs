// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! `wc` — show node content statistics.

use crate::agent::command;
use crate::agent::config::DocContext;
use crate::agent::state::WorkerState;

use super::super::ToolResult;

/// Execute `wc <target>` — show node content statistics.
pub fn wc(target: &str, ctx: &DocContext, state: &WorkerState) -> ToolResult {
    let node_id =
        match command::resolve_target_extended(target, ctx.nav_index, state.current_node, ctx.tree)
        {
            Some(id) => id,
            None => {
                return ToolResult::fail(format!(
                    "Target '{}' not found. Use ls to see available children.",
                    target
                ));
            }
        };

    let content = match ctx.cat(node_id) {
        Some(c) => c,
        None => return ToolResult::fail(format!("No content for '{}'.", target)),
    };

    let title = ctx.node_title(node_id).unwrap_or("unknown");
    let lines = content.lines().count();
    let words = content.split_whitespace().count();
    let chars = content.len();

    ToolResult::ok(format!(
        "[{}] {} lines, {} words, {} chars",
        title, lines, words, chars
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::config::DocContext;
    use crate::agent::state::WorkerState;
    use vectorless_document::{ChildRoute, DocumentTree, NavigationIndex, NodeId};

    fn build_rich_tree() -> (DocumentTree, NavigationIndex, NodeId) {
        let mut tree = DocumentTree::new(
            "Root",
            "Welcome to the financial report.\nThis document covers 2024 and 2023 figures.",
        );
        let root = tree.root();
        let c1 = tree.add_child(
            root,
            "Revenue",
            "Total revenue in 2024 was $10.2M.\nQ1 revenue: $2.5M\nQ2 revenue: $2.8M\nEBITDA margin: 32%",
        );

        let mut nav = NavigationIndex::new();
        nav.add_child_routes(
            root,
            vec![ChildRoute {
                node_id: c1,
                title: "Revenue".to_string(),
                description: "Revenue breakdown".to_string(),
                leaf_count: 2,
            }],
        );

        (tree, nav, root)
    }

    macro_rules! rich_ctx {
        ($tree:expr, $nav:expr) => {
            DocContext {
                tree: &$tree,
                nav_index: &$nav,
                reasoning_index: &vectorless_document::ReasoningIndex::default(),
                doc_name: "test",
            }
        };
    }

    #[test]
    fn test_wc_stats() {
        let (tree, nav, root) = build_rich_tree();
        let ctx = rich_ctx!(tree, nav);
        let state = WorkerState::new(root, 15);

        let result = wc("Revenue", &ctx, &state);
        assert!(result.success);
        assert!(result.feedback.contains("Revenue"));
        assert!(result.feedback.contains("lines"));
        assert!(result.feedback.contains("words"));
        assert!(result.feedback.contains("chars"));
    }

    #[test]
    fn test_wc_not_found() {
        let (tree, nav, root) = build_rich_tree();
        let ctx = rich_ctx!(tree, nav);
        let state = WorkerState::new(root, 15);

        let result = wc("NonExistent", &ctx, &state);
        assert!(!result.success);
    }
}
