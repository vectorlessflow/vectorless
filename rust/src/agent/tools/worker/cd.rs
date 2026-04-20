// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! `cd`, `cd_absolute`, `cd_up` — navigation commands.

use crate::agent::command;
use crate::agent::config::DocContext;
use crate::agent::state::WorkerState;

use super::super::ToolResult;

/// Execute `cd <target>` — navigate into a child node.
///
/// Supports:
/// - Relative names (child of current node): `cd "Getting Started"`
/// - Absolute paths starting with `/`: `cd /root/Chapter 1/Section 1.2`
pub fn cd(target: &str, ctx: &DocContext, state: &mut WorkerState) -> ToolResult {
    if target.starts_with('/') {
        return cd_absolute(target, ctx, state);
    }

    match command::resolve_target_extended(target, ctx.nav_index, state.current_node, ctx.tree) {
        Some(node_id) => {
            let title = ctx.node_title(node_id).unwrap_or(target).to_string();
            state.cd(node_id, &title);
            ToolResult::ok(format!("Entered: {}", state.path_str()))
        }
        None => ToolResult::fail(format!(
            "Target '{}' not found. Use ls to see available children.",
            target
        )),
    }
}

/// Navigate using an absolute path (e.g., `/root/Chapter 1/Section 1.2`).
fn cd_absolute(path: &str, ctx: &DocContext, state: &mut WorkerState) -> ToolResult {
    let segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();

    if segments.is_empty() {
        return ToolResult::fail("Empty absolute path.".to_string());
    }

    let root = ctx.root();
    let mut current = root;

    let start_idx = if !segments.is_empty() && segments[0].eq_ignore_ascii_case("root") {
        1
    } else {
        0
    };

    let mut breadcrumb = vec!["root".to_string()];

    for segment in &segments[start_idx..] {
        match command::resolve_target_extended(segment, ctx.nav_index, current, ctx.tree) {
            Some(node_id) => {
                let title = ctx.node_title(node_id).unwrap_or(*segment).to_string();
                breadcrumb.push(title);
                current = node_id;
            }
            None => {
                return ToolResult::fail(format!(
                    "Path segment '{}' not found. Stopped at: /{}",
                    segment,
                    breadcrumb.join("/")
                ));
            }
        }
    }

    state.breadcrumb = breadcrumb;
    state.current_node = current;
    state.visited.insert(current);

    ToolResult::ok(format!("Entered: {}", state.path_str()))
}

/// Execute `cd ..` — navigate back to parent.
pub fn cd_up(ctx: &DocContext, state: &mut WorkerState) -> ToolResult {
    match ctx.parent(state.current_node) {
        Some(parent) => {
            if state.cd_up(parent) {
                ToolResult::ok(format!("Back to: {}", state.path_str()))
            } else {
                ToolResult::ok("Already at root.".to_string())
            }
        }
        None => ToolResult::ok("Already at root (no parent).".to_string()),
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
    fn test_cd_navigates() {
        let (tree, nav, root, c1, _) = build_test_tree();
        let ctx = DocContext {
            tree: &tree,
            nav_index: &nav,
            reasoning_index: &crate::document::ReasoningIndex::default(),
            doc_name: "test",
        };
        let mut state = WorkerState::new(root, 8);

        let result = cd("Getting Started", &ctx, &mut state);
        assert!(result.success);
        assert_eq!(state.current_node, c1);
        assert!(state.path_str().contains("Getting Started"));
    }

    #[test]
    fn test_cd_up_goes_back() {
        let (tree, nav, root, _c1, _) = build_test_tree();
        let ctx = DocContext {
            tree: &tree,
            nav_index: &nav,
            reasoning_index: &crate::document::ReasoningIndex::default(),
            doc_name: "test",
        };
        let mut state = WorkerState::new(root, 8);

        cd("Getting Started", &ctx, &mut state);
        let result = cd_up(&ctx, &mut state);
        assert!(result.success);
        assert_eq!(state.current_node, root);
    }
}
