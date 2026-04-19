// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! `find_tree` — search for nodes by title pattern across the entire tree.

use crate::agent::config::DocContext;

use super::super::ToolResult;

/// Execute `findtree <pattern>` — search for nodes by title pattern across the entire tree.
///
/// Returns all nodes whose title contains the pattern (case-insensitive).
pub fn find_tree(pattern: &str, ctx: &DocContext) -> ToolResult {
    let pattern_lower = pattern.to_lowercase();
    let all_nodes = ctx.tree.traverse();

    let mut results = Vec::new();
    for node_id in &all_nodes {
        if let Some(node) = ctx.tree.get(*node_id) {
            if node.title.to_lowercase().contains(&pattern_lower) {
                let depth = ctx.tree.depth(*node_id);
                let leaf_count = ctx.nav_entry(*node_id).map(|e| e.leaf_count).unwrap_or(0);
                results.push((node.title.clone(), depth, leaf_count));
            }
        }
    }

    if results.is_empty() {
        return ToolResult::ok(format!("No nodes matching '{}'.", pattern));
    }

    let mut output = format!("Nodes matching '{}' ({} found):\n", pattern, results.len());
    for (title, depth, leaves) in &results {
        output.push_str(&format!(
            "  - {} (depth {}, {} leaves)\n",
            title, depth, leaves
        ));
    }

    ToolResult::ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::config::DocContext;
    use crate::document::{ChildRoute, DocumentTree, NavigationIndex, NodeId};

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
        let c2 = tree.add_child(
            root,
            "Expenses",
            "Operating expenses totaled $6.8M.\nR&D spending: $3.1M\nMarketing: $1.2M",
        );

        let mut nav = NavigationIndex::new();
        nav.add_child_routes(
            root,
            vec![
                ChildRoute {
                    node_id: c1,
                    title: "Revenue".to_string(),
                    description: "Revenue breakdown".to_string(),
                    leaf_count: 2,
                },
                ChildRoute {
                    node_id: c2,
                    title: "Expenses".to_string(),
                    description: "Cost analysis".to_string(),
                    leaf_count: 2,
                },
            ],
        );

        (tree, nav, root)
    }

    macro_rules! rich_ctx {
        ($tree:expr, $nav:expr) => {
            DocContext {
                tree: &$tree,
                nav_index: &$nav,
                reasoning_index: &crate::document::ReasoningIndex::default(),
                doc_name: "test",
            }
        };
    }

    #[test]
    fn test_find_tree() {
        let (tree, nav, _root) = build_rich_tree();
        let ctx = rich_ctx!(tree, nav);

        let result = find_tree("revenue", &ctx);
        assert!(result.success);
        assert!(result.feedback.contains("Revenue"));
    }

    #[test]
    fn test_find_tree_case_insensitive() {
        let (tree, nav, _root) = build_rich_tree();
        let ctx = rich_ctx!(tree, nav);

        let result = find_tree("EXPENSE", &ctx);
        assert!(result.success);
        assert!(result.feedback.contains("Expenses"));
    }

    #[test]
    fn test_find_tree_no_match() {
        let (tree, nav, _root) = build_rich_tree();
        let ctx = rich_ctx!(tree, nav);

        let result = find_tree("nonexistent_xyz", &ctx);
        assert!(result.success);
        assert!(result.feedback.contains("No nodes matching"));
    }
}
