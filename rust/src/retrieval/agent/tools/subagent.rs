// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! SubAgent tools: ls, cd, cd_up, cat, pwd.

use super::ToolResult;
use crate::retrieval::agent::command;
use crate::retrieval::agent::config::DocContext;
use crate::retrieval::agent::config::Evidence;
use crate::retrieval::agent::state::State;

/// Execute `ls` — list children of the current node.
pub fn ls(ctx: &DocContext, state: &State) -> ToolResult {
    let mut output = String::new();

    // Show NavEntry for current node (overview, question hints)
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
                output.push_str("(leaf node — no children)\nUse cd .. to go back or done to finish.");
                return ToolResult::ok(output);
            }

            for (i, route) in routes.iter().enumerate() {
                output.push_str(&format!(
                    "[{}] {} — {} ({} leaves)\n",
                    i + 1,
                    route.title,
                    route.description,
                    route.leaf_count
                ));
            }
            ToolResult::ok(output)
        }
        None => {
            output.push_str("(no navigation data for this node)\nUse cd .. to go back.");
            ToolResult::ok(output)
        }
    }
}

/// Execute `cd <target>` — navigate into a child node.
///
/// Supports:
/// - Relative names (child of current node): `cd "Getting Started"`
/// - Absolute paths starting with `/`: `cd /root/Chapter 1/Section 1.2`
pub fn cd(target: &str, ctx: &DocContext, state: &mut State) -> ToolResult {
    // Absolute path: starts with /
    if target.starts_with('/') {
        return cd_absolute(target, ctx, state);
    }

    // Relative: resolve from current node
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
fn cd_absolute(path: &str, ctx: &DocContext, state: &mut State) -> ToolResult {
    let segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();

    if segments.is_empty() {
        return ToolResult::fail("Empty absolute path.".to_string());
    }

    // Start from root
    let root = ctx.root();
    let mut current = root;

    // Skip "root" if the first segment matches it
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

    // Update state
    state.breadcrumb = breadcrumb;
    state.current_node = current;
    state.visited.insert(current);

    ToolResult::ok(format!("Entered: {}", state.path_str()))
}

/// Execute `cd ..` — navigate back to parent.
pub fn cd_up(ctx: &DocContext, state: &mut State) -> ToolResult {
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

/// Execute `cat <target>` — read node content and collect as evidence.
pub fn cat(target: &str, ctx: &DocContext, state: &mut State) -> ToolResult {
    // First resolve the target
    let node_id =
        match command::resolve_target_extended(target, ctx.nav_index, state.current_node, ctx.tree)
        {
            Some(id) => id,
            None => {
                // Maybe it's the current node itself — check if target matches
                return ToolResult::fail(format!(
                    "Target '{}' not found. Use ls to see available children.",
                    target
                ));
            }
        };

    // Read content
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

            // Mark as visited
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

/// Execute `pwd` — show current navigation path.
pub fn pwd(state: &State) -> ToolResult {
    ToolResult::ok(format!("Current path: {}", state.path_str()))
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
        let state = State::new(root, 8);

        let result = ls(&ctx, &state);
        assert!(result.success);
        assert!(result.feedback.contains("Getting Started"));
        assert!(result.feedback.contains("API Reference"));
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
        let mut state = State::new(root, 8);

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
        let mut state = State::new(root, 8);

        cd("Getting Started", &ctx, &mut state);
        let result = cd_up(&ctx, &mut state);
        assert!(result.success);
        assert_eq!(state.current_node, root);
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
        let mut state = State::new(root, 8);

        let result = cat("Getting Started", &ctx, &mut state);
        assert!(result.success);
        assert!(result.feedback.contains("Evidence collected"));
        assert_eq!(state.evidence.len(), 1);
        assert_eq!(state.evidence[0].content, "gs content");
    }

    #[test]
    fn test_pwd() {
        let (tree, nav, root, _, _) = build_test_tree();
        let ctx = DocContext {
            tree: &tree,
            nav_index: &nav,
            reasoning_index: &crate::document::ReasoningIndex::default(),
            doc_name: "test",
        };
        let mut state = State::new(root, 8);
        cd("API Reference", &ctx, &mut state);

        let result = pwd(&state);
        assert!(result.success);
        assert!(result.feedback.contains("API Reference"));
    }
}
