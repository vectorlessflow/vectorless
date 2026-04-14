// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Enrich stage - Add metadata to the tree.

use super::async_trait;
use std::time::Instant;
use tracing::info;

use crate::document::{DocumentTree, NodeId, ReferenceExtractor, TocView};
use crate::error::Result;

use super::{AccessPattern, IndexStage, StageResult};
use crate::index::pipeline::IndexContext;

/// Enrich stage - adds metadata to the tree.
pub struct EnrichStage;

impl EnrichStage {
    /// Create a new enrich stage.
    pub fn new() -> Self {
        Self
    }

    /// Calculate page ranges for all nodes.
    fn calculate_page_ranges(tree: &mut DocumentTree) {
        // Propagate page ranges up the tree
        Self::propagate_page_ranges(tree, tree.root());
    }

    /// Recursively propagate page ranges from children to parent.
    fn propagate_page_ranges(tree: &mut DocumentTree, node_id: NodeId) {
        let children = tree.children(node_id);

        if children.is_empty() {
            return;
        }

        // First, propagate to all children
        for child_id in &children {
            Self::propagate_page_ranges(tree, *child_id);
        }

        // Then calculate this node's range from children
        let mut min_page: Option<usize> = None;
        let mut max_page: Option<usize> = None;

        for child_id in &children {
            if let Some(child) = tree.get(*child_id) {
                if let Some(start) = child.start_page {
                    min_page = Some(min_page.map_or(start, |m| m.min(start)));
                }
                if let Some(end) = child.end_page {
                    max_page = Some(max_page.map_or(end, |m| m.max(end)));
                }
            }
        }

        // Update this node's page range
        if let (Some(min), Some(max)) = (min_page, max_page) {
            tree.set_page_boundaries(node_id, min, max);
        }
    }

    /// Calculate token statistics.
    fn calculate_token_stats(tree: &DocumentTree) -> (usize, usize) {
        let mut total_tokens = 0;
        let mut node_count = 0;

        for node_id in tree.traverse() {
            if let Some(node) = tree.get(node_id) {
                total_tokens += node.token_count.unwrap_or(0);
                node_count += 1;
            }
        }

        (total_tokens, node_count)
    }

    /// Generate document description from root summary.
    fn generate_description(&self, ctx: &mut IndexContext) {
        if !ctx.options.generate_description {
            return;
        }

        // Use root summary if available
        if let Some(tree) = &ctx.tree {
            if let Some(root) = tree.get(tree.root()) {
                if !root.summary.is_empty() {
                    ctx.description = Some(root.summary.clone());
                    info!("Using root summary as document description");
                }
            }
        }
    }

    /// Extract and resolve in-document cross-references for all nodes.
    ///
    /// Parses content for patterns like "see Section 2.1", "Appendix G", etc.
    /// and resolves them to actual `NodeId`s in the tree using the retrieval
    /// index for fast lookup.
    fn resolve_references(tree: &mut DocumentTree) -> usize {
        let retrieval_index = tree.build_retrieval_index();
        let node_ids: Vec<NodeId> = tree.traverse().into_iter().collect();
        let mut total_resolved = 0;

        for node_id in node_ids {
            let content = tree
                .get(node_id)
                .map(|n| n.content.clone())
                .unwrap_or_default();
            if content.is_empty() {
                continue;
            }

            // Quick check: skip nodes without any reference-like patterns
            let content_lower = content.to_lowercase();
            let has_ref_pattern = content_lower.contains("section")
                || content_lower.contains("appendix")
                || content_lower.contains("table")
                || content_lower.contains("figure")
                || content_lower.contains("page")
                || content_lower.contains("equation");

            if !has_ref_pattern {
                continue;
            }

            let refs = ReferenceExtractor::extract_and_resolve(&content, tree, &retrieval_index);
            let resolved = refs.iter().filter(|r| r.is_resolved()).count();
            if resolved > 0 {
                total_resolved += resolved;
            }
            tree.set_references(node_id, refs);
        }

        total_resolved
    }
}

impl Default for EnrichStage {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl IndexStage for EnrichStage {
    fn name(&self) -> &'static str {
        "enrich"
    }

    fn depends_on(&self) -> Vec<&'static str> {
        vec!["build"]
    }

    fn access_pattern(&self) -> AccessPattern {
        AccessPattern {
            reads_tree: true,
            writes_tree: true, // sets page_boundaries
            writes_description: true,
            ..Default::default()
        }
    }

    async fn execute(&mut self, ctx: &mut IndexContext) -> Result<StageResult> {
        let start = Instant::now();

        let tree = ctx
            .tree
            .as_mut()
            .ok_or_else(|| crate::Error::IndexBuild("Tree not built".to_string()))?;

        // 1. Calculate page ranges
        Self::calculate_page_ranges(tree);
        info!("Calculated page ranges for all nodes");

        // 2. Generate ToC view (cached in context)
        let toc_view = TocView::new();
        let toc = toc_view.generate(tree);
        let _toc_markdown = toc_view.format_markdown(&toc);
        // Could store ToC in context if needed

        // 3. Calculate token statistics
        let (total_tokens, node_count) = Self::calculate_token_stats(tree);
        info!("Total tokens: {}, nodes: {}", total_tokens, node_count);

        // 4. Extract and resolve cross-references
        let resolved_refs = Self::resolve_references(tree);
        if resolved_refs > 0 {
            info!("Resolved {} cross-references", resolved_refs);
        }

        // 5. Generate document description
        self.generate_description(ctx);

        let duration = start.elapsed().as_millis() as u64;
        ctx.metrics.record_enrich(duration);

        info!("Enriched tree metadata in {}ms", duration);

        let mut stage_result = StageResult::success("enrich");
        stage_result.duration_ms = duration;
        stage_result
            .metadata
            .insert("total_tokens".to_string(), serde_json::json!(total_tokens));
        stage_result
            .metadata
            .insert("node_count".to_string(), serde_json::json!(node_count));
        stage_result.metadata.insert(
            "resolved_references".to_string(),
            serde_json::json!(resolved_refs),
        );

        Ok(stage_result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::RefType;

    #[test]
    fn test_resolve_references_section_ref() {
        let mut tree = DocumentTree::new("Root", "root content");
        let s1 = tree.add_child(tree.root(), "Introduction", "Introduction text.");
        tree.set_structure(s1, "1");
        let s2 = tree.add_child(
            tree.root(),
            "Details",
            "For details, see Section 1 for more info",
        );
        tree.set_structure(s2, "2");

        let resolved = EnrichStage::resolve_references(&mut tree);
        assert_eq!(resolved, 1);

        // Verify the reference was stored on s2 and resolved to s1
        let refs = tree.get(s2).unwrap().references.clone();
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].ref_type, RefType::Section);
        assert_eq!(refs[0].target_node, Some(s1));
    }

    #[test]
    fn test_resolve_references_no_refs() {
        let mut tree = DocumentTree::new("Root", "root content");
        tree.add_child(tree.root(), "Section 1", "No references here.");

        let resolved = EnrichStage::resolve_references(&mut tree);
        assert_eq!(resolved, 0);
    }
}
