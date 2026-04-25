// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Route Pass — builds the query routing table from question hints and topic tags.
//!
//! This pass runs after NavigationPass. It reads the tree's question_hints and
//! routing_keywords fields (populated by EnhancePass) and generates a
//! [`QueryRoutingTable`] that lets Agents skip root-level exploration.

use std::collections::HashMap;
use std::time::Instant;
use tracing::{info, warn};

use vectorless_document::{ConceptRoute, DocumentTree, NodeId, QueryRoutingTable, RouteTarget};
use vectorless_error::Result;

use crate::passes::async_trait;
use crate::passes::{AccessPattern, CompilePass, PassResult};
use crate::pipeline::CompileContext;

/// Route Pass — builds the query routing table.
///
/// For each non-leaf node with question_hints, creates RouteTargets from its
/// children. For each unique topic tag, creates a ConceptRoute entry.
pub struct RoutePass;

impl RoutePass {
    /// Create a new route pass.
    pub fn new() -> Self {
        Self
    }

    /// Build route targets from a node's children.
    fn build_child_routes(tree: &DocumentTree, parent_id: NodeId) -> Vec<RouteTarget> {
        let children: Vec<_> = tree.children_iter(parent_id).collect();
        let mut targets = Vec::with_capacity(children.len());

        for child_id in children {
            let node = match tree.get(child_id) {
                Some(n) => n,
                None => continue,
            };

            // Relevance based on question hints count and content richness
            let hint_count = node.question_hints.len();
            let has_content = !node.content.is_empty();
            let relevance = if hint_count > 0 && has_content {
                0.7 + (hint_count as f64).min(3.0) * 0.1
            } else if has_content {
                0.5
            } else {
                0.3
            };

            let reason = if hint_count > 0 {
                format!(
                    "Can answer: {}",
                    node.question_hints.first().unwrap_or(&String::new())
                )
            } else {
                format!("Section: {}", node.title)
            };

            targets.push(RouteTarget {
                node_id: child_id,
                relevance,
                reason,
            });
        }

        // Sort by relevance descending
        targets.sort_by(|a, b| {
            b.relevance
                .partial_cmp(&a.relevance)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        targets
    }

    /// Build concept routes from all topic tags in the tree.
    fn build_concept_routes(tree: &DocumentTree) -> Vec<ConceptRoute> {
        let all_nodes = tree.traverse();
        let mut concept_map: HashMap<String, Vec<RouteTarget>> = HashMap::new();

        for node_id in &all_nodes {
            let node = match tree.get(*node_id) {
                Some(n) => n,
                None => continue,
            };

            for tag in &node.routing_keywords {
                let relevance = if tree.is_leaf(*node_id) { 0.9 } else { 0.7 };
                concept_map
                    .entry(tag.to_lowercase())
                    .or_default()
                    .push(RouteTarget {
                        node_id: *node_id,
                        relevance,
                        reason: format!("Tagged with: {}", tag),
                    });
            }
        }

        // Convert to ConceptRoute, sort targets by relevance
        let mut routes: Vec<ConceptRoute> = concept_map
            .into_iter()
            .map(|(concept, mut targets)| {
                targets.sort_by(|a, b| {
                    b.relevance
                        .partial_cmp(&a.relevance)
                        .unwrap_or(std::cmp::Ordering::Equal)
                });
                targets.truncate(10); // limit per concept
                ConceptRoute { concept, targets }
            })
            .collect();

        routes.sort_by(|a, b| b.targets.len().cmp(&a.targets.len()));
        routes.truncate(50); // limit total concepts
        routes
    }
}

impl Default for RoutePass {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl CompilePass for RoutePass {
    fn name(&self) -> &'static str {
        "route"
    }

    fn depends_on(&self) -> Vec<&'static str> {
        vec!["navigation_index"]
    }

    fn is_optional(&self) -> bool {
        true
    }

    fn access_pattern(&self) -> AccessPattern {
        AccessPattern {
            reads_tree: true,
            writes_query_routes: true,
            ..Default::default()
        }
    }

    async fn execute(&mut self, ctx: &mut CompileContext) -> Result<PassResult> {
        let start = Instant::now();

        let tree = match ctx.tree.as_ref() {
            Some(t) => t,
            None => {
                warn!("[route] No tree, cannot build routing table");
                return Ok(PassResult::failure("route", "Tree not built"));
            }
        };

        let all_nodes = tree.traverse();
        info!(
            "[route] Building routing table for {} nodes",
            all_nodes.len()
        );

        let mut table = QueryRoutingTable::new();

        // Phase 1: Build intent routes from nodes with question hints
        let mut intent_count = 0;
        for &node_id in &all_nodes {
            let node = match tree.get(node_id) {
                Some(n) => n,
                None => continue,
            };

            if node.question_hints.is_empty() {
                continue;
            }

            let targets = Self::build_child_routes(tree, node_id);
            if !targets.is_empty() {
                table.add_intent_route(node_id, targets);
                intent_count += 1;
            }
        }

        // Phase 2: Build concept routes from topic tags
        let concept_routes = Self::build_concept_routes(tree);
        let concept_count = concept_routes.len();
        for route in concept_routes {
            table.add_concept_route(route);
        }

        let duration = start.elapsed().as_millis() as u64;

        info!(
            "[route] Complete: {} intent routes, {} concept routes in {}ms",
            intent_count, concept_count, duration,
        );

        ctx.metrics
            .record_route(duration, intent_count, concept_count);

        ctx.query_routes = Some(table);

        let mut result = PassResult::success("route");
        result.duration_ms = duration;
        result
            .metadata
            .insert("intent_routes".to_string(), serde_json::json!(intent_count));
        result.metadata.insert(
            "concept_routes".to_string(),
            serde_json::json!(concept_count),
        );

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn build_test_tree_with_hints() -> DocumentTree {
        let mut tree = DocumentTree::new("Root", "root content");
        let root = tree.root();

        let sec1 = tree.add_child(root, "Revenue Q3", "Q3 revenue was $4.2B");
        let sec2 = tree.add_child(root, "Revenue Q4", "Q4 revenue was $5.1B");

        // Add question hints
        if let Some(n) = tree.get_mut(root) {
            n.question_hints = vec!["What was the revenue?".to_string()];
            n.routing_keywords = vec!["revenue".to_string(), "finance".to_string()];
        }
        if let Some(n) = tree.get_mut(sec1) {
            n.routing_keywords = vec!["revenue".to_string(), "Q3".to_string()];
        }
        if let Some(n) = tree.get_mut(sec2) {
            n.question_hints = vec!["What was Q4 revenue?".to_string()];
            n.routing_keywords = vec!["revenue".to_string(), "Q4".to_string()];
        }

        tree
    }

    #[test]
    fn test_stage_config() {
        let pass = RoutePass::new();
        assert_eq!(pass.name(), "route");
        assert!(pass.is_optional());
        assert_eq!(pass.depends_on(), vec!["navigation_index"]);

        let ap = pass.access_pattern();
        assert!(ap.reads_tree);
        assert!(ap.writes_query_routes);
        assert!(!ap.writes_tree);
    }

    #[test]
    fn test_build_child_routes_basic() {
        let tree = build_test_tree_with_hints();
        let root = tree.root();
        let targets = RoutePass::build_child_routes(&tree, root);

        assert_eq!(targets.len(), 2);
        // Should be sorted by relevance descending
        assert!(targets[0].relevance >= targets[1].relevance);
    }

    #[test]
    fn test_build_child_routes_with_hints() {
        let tree = build_test_tree_with_hints();
        let root = tree.root();

        // Root has question hints, so build routes from its children
        let targets = RoutePass::build_child_routes(&tree, root);
        assert!(!targets.is_empty());

        // At least one child should have content-based reason
        let has_section_reason = targets.iter().any(|t| t.reason.starts_with("Section:"));
        assert!(has_section_reason);
    }

    #[test]
    fn test_build_concept_routes() {
        let tree = build_test_tree_with_hints();
        let routes = RoutePass::build_concept_routes(&tree);

        assert!(!routes.is_empty());

        // "revenue" appears on all 3 nodes
        let revenue_route = routes.iter().find(|r| r.concept == "revenue");
        assert!(revenue_route.is_some());
        assert!(revenue_route.unwrap().targets.len() >= 2);
    }

    #[test]
    fn test_build_concept_routes_empty() {
        let tree = DocumentTree::new("Root", "no keywords");
        let routes = RoutePass::build_concept_routes(&tree);
        assert!(routes.is_empty());
    }

    #[tokio::test]
    async fn test_execute_end_to_end() {
        let tree = build_test_tree_with_hints();

        let mut ctx = CompileContext::new(
            crate::pipeline::CompilerInput::content("test"),
            crate::config::PipelineOptions::default(),
        );
        ctx.tree = Some(tree);

        let mut pass = RoutePass::new();
        let result = pass.execute(&mut ctx).await;

        assert!(result.is_ok());
        let pass_result = result.unwrap();
        assert!(pass_result.success);

        // Verify routing table
        let table = ctx.query_routes.unwrap();
        assert!(table.intent_route_count() > 0);
        assert!(table.concept_route_count() > 0);

        // Verify metrics recorded
        assert!(ctx.metrics.route_time_ms > 0);
    }

    #[tokio::test]
    async fn test_execute_no_tree() {
        let mut ctx = CompileContext::new(
            crate::pipeline::CompilerInput::content("test"),
            crate::config::PipelineOptions::default(),
        );
        ctx.tree = None;

        let mut pass = RoutePass::new();
        let result = pass.execute(&mut ctx).await.unwrap();
        assert!(!result.success);
        assert!(ctx.query_routes.is_none());
    }

    #[tokio::test]
    async fn test_execute_no_hints_no_keywords() {
        let tree = DocumentTree::new("Root", "plain content");
        let mut ctx = CompileContext::new(
            crate::pipeline::CompilerInput::content("test"),
            crate::config::PipelineOptions::default(),
        );
        ctx.tree = Some(tree);

        let mut pass = RoutePass::new();
        let result = pass.execute(&mut ctx).await;

        assert!(result.is_ok());
        let pass_result = result.unwrap();
        assert!(pass_result.success);

        let table = ctx.query_routes.unwrap();
        assert_eq!(table.intent_route_count(), 0);
        assert_eq!(table.concept_route_count(), 0);
    }
}
