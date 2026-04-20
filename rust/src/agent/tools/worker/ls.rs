// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! `ls` — list children of the current node.

use crate::agent::config::DocContext;
use crate::agent::state::WorkerState;

use super::super::ToolResult;

/// Execute `ls` — list children of the current node.
pub fn ls(ctx: &DocContext, state: &WorkerState) -> ToolResult {
    let mut output = String::new();

    if let Some(entry) = ctx.nav_entry(state.current_node) {
        output.push_str(&format!("Current section: {}\n", entry.overview));
        if !entry.question_hints.is_empty() {
            output.push_str(&format!(
                "Can answer: {}\n",
                entry.question_hints.join(", ")
            ));
        }
        output.push('\n');
    }

    match ctx.ls(state.current_node) {
        Some(routes) => {
            if routes.is_empty() {
                output
                    .push_str("(leaf node — no children)\nUse cd .. to go back or done to finish.");
                return ToolResult::ok(output);
            }

            for (i, route) in routes.iter().enumerate() {
                output.push_str(&format!(
                    "[{}] {} — {} ({} leaves)",
                    i + 1,
                    route.title,
                    route.description,
                    route.leaf_count
                ));
                if let Some(nav) = ctx.nav_entry(route.node_id) {
                    if !nav.question_hints.is_empty() {
                        output.push_str(&format!(
                            "\n    Can answer: {}",
                            nav.question_hints.join(", ")
                        ));
                    }
                    if !nav.topic_tags.is_empty() {
                        output.push_str(&format!("\n    Topics: {}", nav.topic_tags.join(", ")));
                    }
                }
                output.push('\n');
            }
            ToolResult::ok(output)
        }
        None => {
            output.push_str(
                "(no navigation data for this node)\nUse cat to read content or cd .. to go back.",
            );
            ToolResult::ok(output)
        }
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
    fn test_ls_shows_children() {
        let (tree, nav, root, _, _) = build_test_tree();
        let ctx = DocContext {
            tree: &tree,
            nav_index: &nav,
            reasoning_index: &crate::document::ReasoningIndex::default(),
            doc_name: "test",
        };
        let state = WorkerState::new(root, 8);

        let result = ls(&ctx, &state);
        assert!(result.success);
        assert!(result.feedback.contains("Getting Started"));
        assert!(result.feedback.contains("API Reference"));
    }
}
