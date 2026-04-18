// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Navigation Index Stage — Build the Agent navigation index from the document tree.
//!
//! This stage runs after EnrichStage and ReasoningIndexStage. It reads the
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
use tracing::info;

use crate::document::{ChildRoute, DocumentTree, NavEntry, NavigationIndex, NodeId};
use crate::error::Result;

use super::async_trait;
use super::{AccessPattern, IndexStage, StageResult};
use crate::index::pipeline::IndexContext;

/// Navigation Index Stage — builds the Agent navigation index.
///
/// For every non-leaf node in the tree, this stage creates:
/// - A [`NavEntry`] with overview, question hints, topic tags, leaf count, and level.
/// - A list of [`ChildRoute`] entries, one per child, with title, description, and leaf count.
///
/// The resulting [`NavigationIndex`] is stored in `ctx.navigation_index` and
/// serialized as part of [`PersistedDocument`](crate::storage::persistence::PersistedDocument).
pub struct NavigationIndexStage;

impl NavigationIndexStage {
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
                }
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
            question_hints: Vec::new(), // Will be populated when Enhance extracts these
            topic_tags: Vec::new(),     // Will be populated when Enhance adds routing_keywords
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

impl Default for NavigationIndexStage {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl IndexStage for NavigationIndexStage {
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

    async fn execute(&mut self, ctx: &mut IndexContext) -> Result<StageResult> {
        let start = Instant::now();

        let tree = match ctx.tree.as_ref() {
            Some(t) => t,
            None => {
                return Ok(StageResult::failure("navigation_index", "Tree not built"));
            }
        };

        info!("Building navigation index...");

        let all_nodes = tree.traverse();
        let mut nav_entries_count = 0usize;
        let mut child_routes_count = 0usize;

        // Phase 1: Pre-compute leaf counts for all nodes.
        // We compute once per node to avoid repeated traversals.
        let mut leaf_counts: std::collections::HashMap<NodeId, usize> =
            std::collections::HashMap::with_capacity(all_nodes.len());
        for &node_id in &all_nodes {
            leaf_counts.insert(node_id, Self::count_leaves(tree, node_id));
        }

        // Phase 2: Build NavEntry + ChildRoutes for each non-leaf node.
        let mut nav_index = NavigationIndex::new();

        for &node_id in &all_nodes {
            // Skip leaf nodes — they have no children to navigate to
            if tree.is_leaf(node_id) {
                continue;
            }

            let leaf_count = leaf_counts.get(&node_id).copied().unwrap_or(0);

            // Build navigation entry for this non-leaf node
            let nav_entry = Self::build_nav_entry(tree, node_id, leaf_count);
            nav_index.add_entry(node_id, nav_entry);
            nav_entries_count += 1;

            // Build child routes for this node's children
            let child_ids: Vec<NodeId> = tree.children_iter(node_id).collect();
            let mut routes = Vec::with_capacity(child_ids.len());

            for child_id in child_ids {
                let child_leaf_count = leaf_counts.get(&child_id).copied().unwrap_or(0);
                let route = Self::build_child_route(tree, child_id, child_leaf_count);
                routes.push(route);
                child_routes_count += 1;
            }

            nav_index.add_child_routes(node_id, routes);
        }

        let duration = start.elapsed().as_millis() as u64;

        ctx.metrics.record_navigation_index(
            duration,
            nav_entries_count,
            child_routes_count,
        );

        info!(
            "Navigation index built in {}ms ({} nav entries, {} child routes)",
            duration, nav_entries_count, child_routes_count,
        );

        ctx.navigation_index = Some(nav_index);

        let mut stage_result = StageResult::success("navigation_index");
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
    use crate::document::DocumentTree;

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
        assert_eq!(NavigationIndexStage::count_leaves(&tree, root), 3);
    }

    #[test]
    fn test_count_leaves_single_node() {
        let tree = DocumentTree::new("Root", "content");
        let root = tree.root();

        assert_eq!(NavigationIndexStage::count_leaves(&tree, root), 1);
    }

    #[test]
    fn test_build_nav_entry_with_summary() {
        let tree = build_test_tree();
        let root = tree.root();

        let entry = NavigationIndexStage::build_nav_entry(&tree, root, 3);
        assert_eq!(entry.overview, "A comprehensive guide");
        assert_eq!(entry.leaf_count, 3);
        assert_eq!(entry.level, 0);
    }

    #[test]
    fn test_build_nav_entry_without_summary() {
        let tree = DocumentTree::new("Root", "content");
        let root = tree.root();

        let entry = NavigationIndexStage::build_nav_entry(&tree, root, 1);
        assert_eq!(entry.overview, "Root");
    }

    #[test]
    fn test_build_child_route() {
        let tree = build_test_tree();
        let root = tree.root();
        let children: Vec<_> = tree.children_iter(root).collect();

        let route = NavigationIndexStage::build_child_route(&tree, children[0], 2);
        assert_eq!(route.title, "Section 1");
        assert_eq!(route.leaf_count, 2);
    }

    #[test]
    fn test_stage_config() {
        let stage = NavigationIndexStage::new();
        assert_eq!(stage.name(), "navigation_index");
        assert!(stage.is_optional());
        assert_eq!(stage.depends_on(), vec!["enrich"]);

        let ap = stage.access_pattern();
        assert!(ap.reads_tree);
        assert!(ap.writes_navigation_index);
        assert!(!ap.writes_tree);
        assert!(!ap.writes_reasoning_index);
    }
}
