// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Navigation Index Stage — Build the Agent navigation index from the document tree.
//!
//! This stage runs after EnrichPass and ReasoningPass. It reads the
//! enhanced TreeNode fields (summary, description, routing_keywords, leaf_count)
//! and builds a [`NavigationIndex`] containing compact [`NavEntry`] and
//! [`ChildRoute`] records for every non-leaf node.
//!
//! # No LLM Calls
//!
//! This stage performs pure data organization. All LLM-generated content
//! (summaries, descriptions, keywords) is already on the tree from the
//! Enhance stage. This stage only reads and restructures that data.

use std::time::Instant;
use tracing::{debug, info, warn};

use vectorless_document::{ChildRoute, DocumentTree, NavEntry, NavigationIndex, NodeId};
use vectorless_error::Result;

use crate::passes::async_trait;
use crate::passes::{AccessPattern, CompilePass, PassResult};
use crate::pipeline::CompileContext;

/// Navigation Index Stage — builds the Agent navigation index.
///
/// For every non-leaf node in the tree, this stage creates:
/// - A [`NavEntry`] with overview, question hints, topic tags, leaf count, and level.
/// - A list of [`ChildRoute`] entries, one per child, with title, description, and leaf count.
///
/// The resulting [`NavigationIndex`] is stored in `ctx.navigation_index` and
/// serialized as part of [`PersistedDocument`](vectorless_storage::persistence::PersistedDocument).
pub struct NavigationPass;

impl NavigationPass {
    /// Create a new navigation index stage.
    pub fn new() -> Self {
        Self
    }

    /// Count the number of leaf nodes in a subtree rooted at `node_id`.
    fn count_leaves(tree: &DocumentTree, node_id: NodeId) -> usize {
        if tree.is_leaf(node_id) {
            return 1;
        }
        let mut count = 0;
        let mut stack = vec![node_id];
        while let Some(id) = stack.pop() {
            if tree.is_leaf(id) {
                count += 1;
            } else {
                for child in tree.children_iter(id) {
                    stack.push(child);
                }
            }
        }
        count
    }

    /// Build a NavEntry for a non-leaf node.
    fn build_nav_entry(tree: &DocumentTree, node_id: NodeId, leaf_count: usize) -> NavEntry {
        let node = match tree.get(node_id) {
            Some(n) => n,
            None => {
                return NavEntry {
                    overview: String::new(),
                    question_hints: Vec::new(),
                    topic_tags: Vec::new(),
                    leaf_count: 0,
                    level: 0,
                };
            }
        };

        // Overview: use summary if available, otherwise title
        let overview = if !node.summary.is_empty() {
            node.summary.clone()
        } else {
            node.title.clone()
        };

        NavEntry {
            overview,
            question_hints: node.question_hints.clone(),
            topic_tags: node.routing_keywords.clone(),
            leaf_count,
            level: node.depth,
        }
    }

    /// Build a ChildRoute for a single child node.
    fn build_child_route(tree: &DocumentTree, child_id: NodeId, leaf_count: usize) -> ChildRoute {
        let node = tree.get(child_id);
        let title = node.map(|n| n.title.clone()).unwrap_or_default();
        let description = node
            .and_then(|n| {
                // Use summary as description if available; otherwise use a truncated title
                if !n.summary.is_empty() {
                    Some(n.summary.clone())
                } else if !n.content.is_empty() {
                    // Truncate content as fallback description
                    let s: String = n.content.chars().take(100).collect();
                    Some(s)
                } else {
                    None
                }
            })
            .unwrap_or_else(|| title.clone());

        ChildRoute {
            node_id: child_id,
            title,
            description,
            leaf_count,
        }
    }
}

impl Default for NavigationPass {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl CompilePass for NavigationPass {
    fn name(&self) -> &'static str {
        "navigation_index"
    }

    fn depends_on(&self) -> Vec<&'static str> {
        vec!["enrich"]
    }

    fn is_optional(&self) -> bool {
        true
    }

    fn access_pattern(&self) -> AccessPattern {
        AccessPattern {
            reads_tree: true,
            writes_navigation_index: true,
            ..Default::default()
        }
    }

    async fn execute(&mut self, ctx: &mut CompileContext) -> Result<PassResult> {
        let start = Instant::now();

        let tree = match ctx.tree.as_ref() {
            Some(t) => t,
            None => {
                warn!("[navigation_index] No tree, cannot build index");
                return Ok(PassResult::failure("navigation_index", "Tree not built"));
            }
        };

        let all_nodes = tree.traverse();
        let leaf_count = all_nodes.iter().filter(|&&id| tree.is_leaf(id)).count();
        let non_leaf_count = all_nodes.len() - leaf_count;

        info!(
            "[navigation_index] Starting: {} total nodes ({} leaves, {} non-leaf)",
            all_nodes.len(),
            leaf_count,
            non_leaf_count,
        );

        let mut nav_entries_count = 0usize;
        let mut child_routes_count = 0usize;

        // Phase 1: Pre-compute leaf counts for all nodes.
        // We compute once per node to avoid repeated traversals.
        debug!(
            "[navigation_index] Phase 1: Pre-computing leaf counts for {} nodes",
            all_nodes.len()
        );
        let mut leaf_counts: std::collections::HashMap<NodeId, usize> =
            std::collections::HashMap::with_capacity(all_nodes.len());
        for &node_id in &all_nodes {
            leaf_counts.insert(node_id, Self::count_leaves(tree, node_id));
        }

        // Phase 2: Build NavEntry + ChildRoutes for each non-leaf node.
        debug!(
            "[navigation_index] Phase 2: Building NavEntry + ChildRoutes for {} non-leaf nodes",
            non_leaf_count
        );
        let mut nav_index = NavigationIndex::new();

        for &node_id in &all_nodes {
            // Skip leaf nodes — they have no children to navigate to
            if tree.is_leaf(node_id) {
                continue;
            }

            let lc = *leaf_counts.get(&node_id).unwrap_or(&0);

            // Build navigation entry for this non-leaf node
            let nav_entry = Self::build_nav_entry(tree, node_id, lc);
            nav_index.add_entry(node_id, nav_entry);
            nav_entries_count += 1;

            // Build child routes for this node's children
            let child_ids: Vec<NodeId> = tree.children_iter(node_id).collect();
            let mut routes = Vec::with_capacity(child_ids.len());

            for child_id in child_ids {
                let child_lc = *leaf_counts.get(&child_id).unwrap_or(&0);
                let route = Self::build_child_route(tree, child_id, child_lc);
                routes.push(route);
                child_routes_count += 1;
            }

            debug!(
                "[navigation_index]   node '{}' → {} child routes ({} leaves in subtree)",
                tree.get(node_id).map(|n| n.title.as_str()).unwrap_or("?"),
                routes.len(),
                lc,
            );

            nav_index.add_child_routes(node_id, routes);
        }

        // Phase 3: Build DocCard from root-level data (already computed, zero LLM).
        // Provides a compact document summary for multi-document Orchestrator Agent.
        if let Some(root_entry) = nav_index.get_entry(tree.root()) {
            let sections: Vec<vectorless_document::SectionCard> = nav_index
                .get_child_routes(tree.root())
                .map(|routes| {
                    routes
                        .iter()
                        .map(|r| vectorless_document::SectionCard {
                            title: r.title.clone(),
                            description: r.description.clone(),
                            leaf_count: r.leaf_count,
                        })
                        .collect()
                })
                .unwrap_or_default();

            let doc_card = vectorless_document::DocCard {
                title: tree
                    .get(tree.root())
                    .map(|n| n.title.clone())
                    .unwrap_or_default(),
                overview: root_entry.overview.clone(),
                question_hints: root_entry.question_hints.clone(),
                topic_tags: root_entry.topic_tags.clone(),
                sections,
                total_leaves: root_entry.leaf_count,
            };
            nav_index.set_doc_card(doc_card);

            debug!(
                "[navigation_index] Phase 3: Built DocCard — {} sections, {} total leaves",
                nav_index.doc_card().map(|c| c.sections.len()).unwrap_or(0),
                nav_index.doc_card().map(|c| c.total_leaves).unwrap_or(0),
            );
        } else {
            debug!("[navigation_index] Phase 3: Skipped DocCard (no root entry)");
        }

        let duration = start.elapsed().as_millis() as u64;

        ctx.metrics
            .record_navigation_index(duration, nav_entries_count, child_routes_count);

        info!(
            "[navigation_index] Complete: {} nav entries, {} child routes in {}ms",
            nav_entries_count, child_routes_count, duration,
        );

        ctx.navigation_index = Some(nav_index);

        let mut stage_result = PassResult::success("navigation_index");
        stage_result.duration_ms = duration;
        stage_result.metadata.insert(
            "nav_entries".to_string(),
            serde_json::json!(nav_entries_count),
        );
        stage_result.metadata.insert(
            "child_routes".to_string(),
            serde_json::json!(child_routes_count),
        );

        Ok(stage_result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vectorless_document::DocumentTree;

    fn build_test_tree() -> DocumentTree {
        let mut tree = DocumentTree::new("Root", "root content");
        let root = tree.root();

        let sec1 = tree.add_child(root, "Section 1", "section 1 content");
        let _sec1_1 = tree.add_child(sec1, "Section 1.1", "s1.1 content");
        let _sec1_2 = tree.add_child(sec1, "Section 1.2", "s1.2 content");

        let sec2 = tree.add_child(root, "Section 2", "section 2 content");
        let _sec2_1 = tree.add_child(sec2, "Section 2.1", "s2.1 content");

        // Set some summaries
        tree.set_summary(root, "A comprehensive guide");
        tree.set_summary(sec1, "Getting started with setup");
        tree.set_summary(sec2, "Advanced configuration");

        tree
    }

    #[test]
    fn test_count_leaves() {
        let tree = build_test_tree();
        let root = tree.root();

        // Root has 3 leaves: 1.1, 1.2, 2.1
        assert_eq!(NavigationPass::count_leaves(&tree, root), 3);
    }

    #[test]
    fn test_count_leaves_single_node() {
        let tree = DocumentTree::new("Root", "content");
        let root = tree.root();

        assert_eq!(NavigationPass::count_leaves(&tree, root), 1);
    }

    #[test]
    fn test_build_nav_entry_with_summary() {
        let tree = build_test_tree();
        let root = tree.root();

        let entry = NavigationPass::build_nav_entry(&tree, root, 3);
        assert_eq!(entry.overview, "A comprehensive guide");
        assert_eq!(entry.leaf_count, 3);
        assert_eq!(entry.level, 0);
    }

    #[test]
    fn test_build_nav_entry_without_summary() {
        let tree = DocumentTree::new("Root", "content");
        let root = tree.root();

        let entry = NavigationPass::build_nav_entry(&tree, root, 1);
        assert_eq!(entry.overview, "Root");
    }

    #[test]
    fn test_build_child_route() {
        let tree = build_test_tree();
        let root = tree.root();
        let children: Vec<_> = tree.children_iter(root).collect();

        let route = NavigationPass::build_child_route(&tree, children[0], 2);
        assert_eq!(route.title, "Section 1");
        assert_eq!(route.leaf_count, 2);
    }

    #[test]
    fn test_stage_config() {
        let stage = NavigationPass::new();
        assert_eq!(stage.name(), "navigation_index");
        assert!(stage.is_optional());
        assert_eq!(stage.depends_on(), vec!["enrich"]);

        let ap = stage.access_pattern();
        assert!(ap.reads_tree);
        assert!(ap.writes_navigation_index);
        assert!(!ap.writes_tree);
        assert!(!ap.writes_reasoning_index);
    }

    #[tokio::test]
    async fn test_execute_end_to_end() {
        // Build a 3-level tree: Root -> [Sec1 -> [1.1, 1.2], Sec2 -> [2.1]]
        let mut tree = DocumentTree::new("Root", "root content");
        let root = tree.root();
        let sec1 = tree.add_child(root, "Section 1", "s1 content");
        let _sec1_1 = tree.add_child(sec1, "Section 1.1", "s1.1 content");
        let _sec1_2 = tree.add_child(sec1, "Section 1.2", "s1.2 content");
        let sec2 = tree.add_child(root, "Section 2", "s2 content");
        let _sec2_1 = tree.add_child(sec2, "Section 2.1", "s2.1 content");

        tree.set_summary(root, "A comprehensive guide");
        tree.set_summary(sec1, "Getting started");

        // Build context with the tree
        let mut ctx = CompileContext::new(
            crate::pipeline::CompilerInput::content("test"),
            crate::config::PipelineOptions::default(),
        );
        ctx.tree = Some(tree);

        // Execute the stage
        let mut stage = NavigationPass::new();
        let result = stage.execute(&mut ctx).await;

        assert!(result.is_ok());
        let stage_result = result.unwrap();
        assert!(stage_result.success);
        assert_eq!(
            stage_result.metadata["nav_entries"],
            serde_json::json!(3) // root, sec1, sec2
        );
        assert_eq!(
            stage_result.metadata["child_routes"],
            serde_json::json!(5) // root→2 + sec1→2 + sec2→1
        );

        // Verify the index structure
        let nav_index = ctx.navigation_index.unwrap();
        assert_eq!(nav_index.entry_count(), 3); // 3 non-leaf nodes
        assert_eq!(nav_index.total_child_routes(), 5);

        // Root entry
        let root_id = ctx.tree.as_ref().unwrap().root();
        let root_entry = nav_index.get_entry(root_id).unwrap();
        assert_eq!(root_entry.overview, "A comprehensive guide");
        assert_eq!(root_entry.leaf_count, 3);
        assert_eq!(root_entry.level, 0);

        // Root child routes
        let root_routes = nav_index.get_child_routes(root_id).unwrap();
        assert_eq!(root_routes.len(), 2);
        assert_eq!(root_routes[0].title, "Section 1");
        assert_eq!(root_routes[0].leaf_count, 2);
        assert_eq!(root_routes[1].title, "Section 2");
        assert_eq!(root_routes[1].leaf_count, 1);
    }

    #[tokio::test]
    async fn test_execute_single_leaf_tree() {
        // Single node = root is leaf → no non-leaf nodes → empty index
        let tree = DocumentTree::new("Root", "content");

        let mut ctx = CompileContext::new(
            crate::pipeline::CompilerInput::content("test"),
            crate::config::PipelineOptions::default(),
        );
        ctx.tree = Some(tree);

        let mut stage = NavigationPass::new();
        let result = stage.execute(&mut ctx).await;

        assert!(result.is_ok());
        assert!(stage_result_is_success(&result));

        let nav_index = ctx.navigation_index.unwrap();
        assert_eq!(nav_index.entry_count(), 0);
        assert_eq!(nav_index.total_child_routes(), 0);
    }

    #[tokio::test]
    async fn test_execute_no_tree() {
        let ctx = CompileContext::new(
            crate::pipeline::CompilerInput::content("test"),
            crate::config::PipelineOptions::default(),
        );
        // ctx.tree is None

        let mut stage = NavigationPass::new();
        // Can't move ctx since tree is None, construct manually
        let mut ctx = ctx;
        ctx.tree = None;

        let result = stage.execute(&mut ctx).await.unwrap();
        assert!(!result.success);
        assert!(ctx.navigation_index.is_none());
    }

    #[test]
    fn test_build_child_route_no_summary_has_content() {
        // Node with content but no summary → description = truncated content
        let mut tree = DocumentTree::new("Root", "");
        let root = tree.root();
        let child = tree.add_child(root, "Child", "this is a long content string that exceeds 100 characters and should be truncated when used as a fallback description for the child route");

        let route = NavigationPass::build_child_route(&tree, child, 1);
        assert_eq!(route.title, "Child");
        // description should be truncated content, not the full string
        assert!(route.description.len() <= 100);
        assert!(route.description.starts_with("this is a long"));
    }

    #[test]
    fn test_build_child_route_no_summary_no_content() {
        // Node with neither summary nor content → description = title
        let mut tree = DocumentTree::new("Root", "");
        let root = tree.root();
        let child = tree.add_child(root, "Orphan Section", "");
        // Clear any auto-generated content
        tree.set_summary(child, "");

        let route = NavigationPass::build_child_route(&tree, child, 1);
        assert_eq!(route.title, "Orphan Section");
        // Fallback: description = title when no summary and no content
        assert_eq!(route.description, "Orphan Section");
    }

    #[test]
    fn test_build_child_route_with_summary() {
        let mut tree = DocumentTree::new("Root", "");
        let root = tree.root();
        let child = tree.add_child(root, "Child", "some content");
        tree.set_summary(child, "A concise summary");

        let route = NavigationPass::build_child_route(&tree, child, 1);
        assert_eq!(route.description, "A concise summary");
    }

    #[test]
    fn test_build_nav_entry_depth_tracking() {
        // Verify that depth/level is correctly captured from the tree
        let mut tree = DocumentTree::new("Root", "");
        let root = tree.root();
        let sec1 = tree.add_child(root, "S1", "");
        let sec1_1 = tree.add_child(sec1, "S1.1", "leaf");
        tree.set_summary(root, "Root overview");
        tree.set_summary(sec1, "Section overview");

        let root_entry = NavigationPass::build_nav_entry(&tree, root, 3);
        assert_eq!(root_entry.level, 0);

        let sec1_entry = NavigationPass::build_nav_entry(&tree, sec1, 1);
        assert_eq!(sec1_entry.level, 1);

        // Leaf node should still return valid NavEntry if called
        let leaf_entry = NavigationPass::build_nav_entry(&tree, sec1_1, 1);
        assert_eq!(leaf_entry.level, 2);
        assert_eq!(leaf_entry.overview, "S1.1"); // no summary → fallback to title
    }

    #[test]
    fn test_count_leaves_subtree() {
        // Verify leaf count is correct for a subtree, not the entire tree
        let mut tree = DocumentTree::new("Root", "");
        let root = tree.root();
        let sec1 = tree.add_child(root, "S1", "");
        let _s1a = tree.add_child(sec1, "S1.A", "leaf");
        let _s1b = tree.add_child(sec1, "S1.B", "leaf");
        let _s1c = tree.add_child(sec1, "S1.C", "leaf");
        let sec2 = tree.add_child(root, "S2", "");
        let _s2a = tree.add_child(sec2, "S2.A", "leaf");

        // sec1 subtree has 3 leaves
        assert_eq!(NavigationPass::count_leaves(&tree, sec1), 3);
        // sec2 subtree has 1 leaf
        assert_eq!(NavigationPass::count_leaves(&tree, sec2), 1);
        // root has 4 leaves total
        assert_eq!(NavigationPass::count_leaves(&tree, root), 4);
    }

    /// Helper to check success without destructuring.
    fn stage_result_is_success(result: &Result<PassResult>) -> bool {
        result.as_ref().map(|r| r.success).unwrap_or(false)
    }
}
