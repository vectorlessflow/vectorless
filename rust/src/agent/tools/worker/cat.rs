// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! `cat` — read node content and collect as evidence.

use crate::agent::command;
use crate::agent::config::{DocContext, Evidence};
use crate::agent::state::WorkerState;

use super::super::ToolResult;

/// Execute `cat <target>` — read node content and collect as evidence.
///
/// Special targets:
/// - `cat .` or `cat` (no arg) reads the current node's content.
/// - Otherwise resolves the target to a child node by name.
pub fn cat(target: &str, ctx: &DocContext, state: &mut WorkerState) -> ToolResult {
    let node_id = if target == "." || target.is_empty() {
        state.current_node
    } else {
        match command::resolve_target_extended(
            target,
            ctx.nav_index,
            state.current_node,
            ctx.tree,
        ) {
            Some(id) => id,
            None => {
                return ToolResult::fail(format!(
                    "Target '{}' not found. Use 'ls' to see children, or 'cat .' to read current node.",
                    target
                ));
            }
        }
    };

    if state.visited.contains(&node_id) {
        let title = ctx.node_title(node_id).unwrap_or("unknown");
        return ToolResult::ok(format!(
            "[Already collected: {}]. Use a different target or cd to another branch.",
            title
        ));
    }

    match ctx.cat(node_id) {
        Some(content) => {
            let title = ctx.node_title(node_id).unwrap_or("unknown").to_string();
            let content_string = content.to_string();

            state.add_evidence(Evidence {
                source_path: format!("{}/{}", state.path_str(), title),
                node_title: title.clone(),
                content: content_string.clone(),
                doc_name: Some(ctx.doc_name.to_string()),
            });

            state.visited.insert(node_id);

            let preview = if content_string.len() > 500 {
                format!(
                    "{}...(truncated, {} chars total)",
                    &content_string[..500],
                    content_string.len()
                )
            } else {
                content_string
            };

            ToolResult::ok(format!("[Evidence collected: {}]\n{}", title, preview))
        }
        None => ToolResult::fail(format!("No content available for '{}'.", target)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::{ChildRoute, DocumentTree, NavigationIndex, NodeId};

    fn build_test_tree() -> (DocumentTree, NavigationIndex, NodeId, NodeId, NodeId) {
        let mut tree = DocumentTree::new("Root", "root content");
        let root = tree.root();
        let c1 = tree.add_child(root, "Getting Started", "gs content");
        let c2 = tree.add_child(root, "API Reference", "api content");

        let mut nav = NavigationIndex::new();
        nav.add_child_routes(
            root,
            vec![
                ChildRoute {
                    node_id: c1,
                    title: "Getting Started".to_string(),
                    description: "Setup guide".to_string(),
                    leaf_count: 3,
                },
                ChildRoute {
                    node_id: c2,
                    title: "API Reference".to_string(),
                    description: "API docs".to_string(),
                    leaf_count: 7,
                },
            ],
        );

        (tree, nav, root, c1, c2)
    }

    #[test]
    fn test_cat_collects_evidence() {
        let (tree, nav, root, _, _) = build_test_tree();
        let ctx = DocContext {
            tree: &tree,
            nav_index: &nav,
            reasoning_index: &crate::document::ReasoningIndex::default(),
            doc_name: "test",
        };
        let mut state = WorkerState::new(root, 8);

        let result = cat("Getting Started", &ctx, &mut state);
        assert!(result.success);
        assert!(result.feedback.contains("Evidence collected"));
        assert_eq!(state.evidence.len(), 1);
        assert_eq!(state.evidence[0].content, "gs content");
    }
}
