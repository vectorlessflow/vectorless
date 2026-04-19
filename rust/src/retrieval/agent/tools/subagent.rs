// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! SubAgent tools: ls, cd, cd_up, cat, pwd, grep, head, find_tree, wc.

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

    // Guard: skip if already visited (prevents duplicate evidence)
    if state.visited.contains(&node_id) {
        let title = ctx.node_title(node_id).unwrap_or("unknown");
        return ToolResult::ok(format!(
            "[Already collected: {}]. Use a different target or cd to another branch.",
            title
        ));
    }

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

/// Execute `grep <pattern>` — regex search across all node content in the current subtree.
///
/// Searches content of the current node and all descendants. Returns matching lines
/// with their node titles, capped at 30 matches to avoid overwhelming feedback.
pub fn grep(pattern: &str, ctx: &DocContext, state: &State) -> ToolResult {
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
            output.push_str(&format!("\n... (truncated, more matches available)"));
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
                let preview = if line.len() > 120 {
                    format!("{}...", &line[..120])
                } else {
                    line.to_string()
                };
                output.push_str(&format!("[{}] {}\n", title, preview));
                matches_found += 1;
            }
        }
    }

    if matches_found == 0 {
        ToolResult::ok(format!("No matches for /{}/ in subtree.", pattern))
    } else {
        ToolResult::ok(format!("Found {} match(es) for /{}/:\n{}", matches_found, pattern, output))
    }
}

/// Execute `head <target>` — preview first N lines of a node without collecting evidence.
pub fn head(target: &str, lines: usize, ctx: &DocContext, state: &State) -> ToolResult {
    let node_id = match command::resolve_target_extended(
        target,
        ctx.nav_index,
        state.current_node,
        ctx.tree,
    ) {
        Some(id) => id,
        None => {
            return ToolResult::fail(format!(
                "Target '{}' not found. Use ls to see available children.",
                target
            ))
        }
    };

    let content = match ctx.cat(node_id) {
        Some(c) => c,
        None => return ToolResult::fail(format!("No content for '{}'.", target)),
    };

    let title = ctx.node_title(node_id).unwrap_or("unknown");
    let total_lines = content.lines().count();
    let preview: Vec<&str> = content.lines().take(lines).collect();

    let mut output = format!(
        "[Preview: {} — showing {}/{} lines]\n",
        title,
        preview.len().min(lines),
        total_lines
    );
    output.push_str(&preview.join("\n"));

    if total_lines > lines {
        output.push_str(&format!(
            "\n... ({} more lines, use cat to read all)",
            total_lines - lines
        ));
    }

    ToolResult::ok(output)
}

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
                let leaf_count = ctx
                    .nav_entry(*node_id)
                    .map(|e| e.leaf_count)
                    .unwrap_or(0);
                results.push((node.title.clone(), depth, leaf_count));
            }
        }
    }

    if results.is_empty() {
        return ToolResult::ok(format!("No nodes matching '{}'.", pattern));
    }

    let mut output = format!("Nodes matching '{}' ({} found):\n", pattern, results.len());
    for (title, depth, leaves) in &results {
        output.push_str(&format!("  - {} (depth {}, {} leaves)\n", title, depth, leaves));
    }

    ToolResult::ok(output)
}

/// Execute `wc <target>` — show node content statistics.
pub fn wc(target: &str, ctx: &DocContext, state: &State) -> ToolResult {
    let node_id = match command::resolve_target_extended(
        target,
        ctx.nav_index,
        state.current_node,
        ctx.tree,
    ) {
        Some(id) => id,
        None => {
            return ToolResult::fail(format!(
                "Target '{}' not found. Use ls to see available children.",
                target
            ))
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

/// Collect all NodeIds in the subtree rooted at `node` (inclusive).
fn collect_subtree(node: crate::document::NodeId, tree: &crate::document::DocumentTree) -> Vec<crate::document::NodeId> {
    let mut result = vec![node];
    let mut stack = vec![node];

    while let Some(current) = stack.pop() {
        for child in tree.children_iter(current) {
            result.push(child);
            stack.push(child);
        }
    }

    result
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

    // --- Tests for new tools ---

    /// Build a richer tree with multi-line content for grep/head/wc testing.
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
        let state = State::new(root, 8);

        let result = grep("revenue", &ctx, &state);
        assert!(result.success);
        assert!(result.feedback.contains("revenue"));
        assert!(result.feedback.contains("[Revenue]"));
    }

    #[test]
    fn test_grep_regex() {
        let (tree, nav, root) = build_rich_tree();
        let ctx = rich_ctx!(tree, nav);
        let state = State::new(root, 8);

        let result = grep("EBITDA|\\$\\d+", &ctx, &state);
        assert!(result.success);
        assert!(result.feedback.contains("EBITDA"));
        assert!(result.feedback.contains("$10"));
    }

    #[test]
    fn test_grep_no_matches() {
        let (tree, nav, root) = build_rich_tree();
        let ctx = rich_ctx!(tree, nav);
        let state = State::new(root, 8);

        let result = grep("nonexistent_term_xyz", &ctx, &state);
        assert!(result.success);
        assert!(result.feedback.contains("No matches"));
    }

    #[test]
    fn test_grep_invalid_regex() {
        let (tree, nav, root) = build_rich_tree();
        let ctx = rich_ctx!(tree, nav);
        let state = State::new(root, 8);

        let result = grep("[invalid", &ctx, &state);
        assert!(!result.success);
        assert!(result.feedback.contains("Invalid regex"));
    }

    #[test]
    fn test_grep_subtree_only() {
        let (tree, nav, root) = build_rich_tree();
        let ctx = rich_ctx!(tree, nav);
        let mut state = State::new(root, 8);

        // cd into Expenses — grep should only find expenses content, not revenue
        cd("Expenses", &ctx, &mut state);
        let result = grep("revenue", &ctx, &state);
        assert!(result.success);
        assert!(result.feedback.contains("No matches"));
    }

    #[test]
    fn test_head_preview() {
        let (tree, nav, root) = build_rich_tree();
        let ctx = rich_ctx!(tree, nav);
        let state = State::new(root, 8);

        let result = head("Revenue", 2, &ctx, &state);
        assert!(result.success);
        assert!(result.feedback.contains("Preview"));
        assert!(result.feedback.contains("$10.2M"));
        assert!(result.feedback.contains("2/4 lines"));
        // Should NOT collect evidence
        assert!(state.evidence.is_empty());
    }

    #[test]
    fn test_head_not_found() {
        let (tree, nav, root) = build_rich_tree();
        let ctx = rich_ctx!(tree, nav);
        let state = State::new(root, 8);

        let result = head("NonExistent", 10, &ctx, &state);
        assert!(!result.success);
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

    #[test]
    fn test_wc_stats() {
        let (tree, nav, root) = build_rich_tree();
        let ctx = rich_ctx!(tree, nav);
        let state = State::new(root, 8);

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
        let state = State::new(root, 8);

        let result = wc("NonExistent", &ctx, &state);
        assert!(!result.success);
    }
}
