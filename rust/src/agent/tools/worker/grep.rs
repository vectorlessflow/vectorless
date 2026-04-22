// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! `grep` — regex search across all node content in the current subtree.

use crate::agent::config::DocContext;
use crate::agent::state::WorkerState;

use super::super::ToolResult;
use super::collect_subtree;

/// Execute `grep <pattern>` — regex search across all node content in the current subtree.
///
/// Searches content of the current node and all descendants. Returns matching lines
/// with their node titles, capped at 30 matches to avoid overwhelming feedback.
pub fn grep(pattern: &str, ctx: &DocContext, state: &WorkerState) -> ToolResult {
    let re = match regex::Regex::new(pattern) {
        Ok(re) => re,
        Err(e) => return ToolResult::fail(format!("Invalid regex '{}': {}", pattern, e)),
    };

    let subtree = collect_subtree(state.current_node, ctx.tree);
    let mut matches_found = 0;
    let mut output = String::new();
    let max_matches = 30;

    for node_id in &subtree {
        if matches_found >= max_matches {
            output.push_str("\n... (truncated, more matches available)");
            break;
        }

        let content = match ctx.cat(*node_id) {
            Some(c) if !c.is_empty() => c,
            _ => continue,
        };

        let title = ctx.node_title(*node_id).unwrap_or("?");

        for line in content.lines() {
            if matches_found >= max_matches {
                break;
            }
            if re.is_match(line) {
                output.push_str(&format!("[{}] {}\n", title, line));
                matches_found += 1;
            }
        }
    }

    if matches_found == 0 {
        ToolResult::ok(format!("No matches for /{}/ in subtree.", pattern))
    } else {
        ToolResult::ok(format!(
            "Found {} match(es) for /{}/:\n{}",
            matches_found, pattern, output
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::config::DocContext;
    use crate::agent::state::WorkerState;
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
    fn test_grep_finds_matches() {
        let (tree, nav, root) = build_rich_tree();
        let ctx = rich_ctx!(tree, nav);
        let state = WorkerState::new(root, 15);

        let result = grep("revenue", &ctx, &state);
        assert!(result.success);
        assert!(result.feedback.contains("revenue"));
        assert!(result.feedback.contains("[Revenue]"));
    }

    #[test]
    fn test_grep_regex() {
        let (tree, nav, root) = build_rich_tree();
        let ctx = rich_ctx!(tree, nav);
        let state = WorkerState::new(root, 15);

        let result = grep("EBITDA|\\$\\d+", &ctx, &state);
        assert!(result.success);
        assert!(result.feedback.contains("EBITDA"));
        assert!(result.feedback.contains("$10"));
    }

    #[test]
    fn test_grep_no_matches() {
        let (tree, nav, root) = build_rich_tree();
        let ctx = rich_ctx!(tree, nav);
        let state = WorkerState::new(root, 15);

        let result = grep("nonexistent_term_xyz", &ctx, &state);
        assert!(result.success);
        assert!(result.feedback.contains("No matches"));
    }

    #[test]
    fn test_grep_invalid_regex() {
        let (tree, nav, root) = build_rich_tree();
        let ctx = rich_ctx!(tree, nav);
        let state = WorkerState::new(root, 15);

        let result = grep("[invalid", &ctx, &state);
        assert!(!result.success);
        assert!(result.feedback.contains("Invalid regex"));
    }

    #[test]
    fn test_grep_subtree_only() {
        let (tree, nav, root) = build_rich_tree();
        let ctx = rich_ctx!(tree, nav);
        let mut state = WorkerState::new(root, 15);

        crate::agent::tools::worker::cd::cd("Expenses", &ctx, &mut state);
        let result = grep("revenue", &ctx, &state);
        assert!(result.success);
        assert!(result.feedback.contains("No matches"));
    }
}
