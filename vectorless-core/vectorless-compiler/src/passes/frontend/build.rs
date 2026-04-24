// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Build stage - Build tree from raw nodes.

use crate::passes::async_trait;
use std::time::Instant;
use tracing::{debug, info};

use crate::parse::RawNode;
use vectorless_document::{DocumentTree, NodeId};
use vectorless_error::Result;
use vectorless_utils::estimate_tokens;

use crate::passes::{CompilePass, PassResult};
use crate::ThinningConfig;
use crate::pipeline::CompileContext;

/// Build stage - constructs a tree from raw nodes.
pub struct BuildPass;

impl BuildPass {
    /// Create a new build stage.
    pub fn new() -> Self {
        Self
    }

    /// Calculate total token counts for all nodes (recursive, includes children).
    fn calculate_total_tokens(nodes: &mut [RawNode]) {
        if nodes.is_empty() {
            return;
        }

        // Process from back to front
        for i in (0..nodes.len()).rev() {
            let own_tokens = nodes[i]
                .token_count
                .unwrap_or_else(|| estimate_tokens(&nodes[i].content));
            nodes[i].token_count = Some(own_tokens);

            // Find all children (direct and indirect)
            let children_tokens: usize = Self::find_all_children_indices(i, nodes)
                .iter()
                .map(|&child_idx| nodes[child_idx].total_token_count.unwrap_or(0))
                .sum();

            nodes[i].total_token_count = Some(own_tokens + children_tokens);
        }
    }

    /// Find all children (direct and indirect) of a node.
    fn find_all_children_indices(parent_idx: usize, nodes: &[RawNode]) -> Vec<usize> {
        let parent_level = nodes[parent_idx].level;
        let mut children = Vec::new();

        for i in (parent_idx + 1)..nodes.len() {
            if nodes[i].level <= parent_level {
                break;
            }
            children.push(i);
        }

        children
    }

    /// Find direct children of a node.
    fn find_direct_children_indices(parent_idx: usize, nodes: &[RawNode]) -> Vec<usize> {
        let parent_level = nodes[parent_idx].level;
        let target_level = parent_level + 1;
        let mut children = Vec::new();
        let mut i = parent_idx + 1;

        while i < nodes.len() {
            if nodes[i].level <= parent_level {
                break;
            }
            if nodes[i].level == target_level {
                children.push(i);
            }
            i += 1;
        }

        children
    }

    /// Apply thinning to raw nodes before tree construction.
    ///
    /// When `merge_content` is true: small nodes are merged into their parent
    /// by concatenating child content into the parent, then marking children for removal.
    /// When `merge_content` is false: small nodes are simply marked for removal.
    fn apply_thinning(nodes: &mut [RawNode], config: &ThinningConfig) -> Vec<bool> {
        if !config.enabled || nodes.is_empty() {
            return vec![true; nodes.len()];
        }

        let mut keep = vec![true; nodes.len()];

        // Process from leaves to root (bottom-up)
        for i in (0..nodes.len()).rev() {
            if !keep[i] {
                continue;
            }
            let total_tokens = nodes[i].total_token_count.unwrap_or(0);

            if total_tokens < config.threshold {
                // Find all children of this node
                let children_indices = Self::find_all_children_indices(i, nodes);

                if !children_indices.is_empty() && config.merge_content {
                    // Merge children content into this node
                    let mut merged_content = nodes[i].content.clone();
                    for &child_idx in &children_indices {
                        if !nodes[child_idx].content.trim().is_empty() {
                            if !merged_content.is_empty() {
                                merged_content.push_str("\n\n");
                            }
                            merged_content.push_str(&nodes[child_idx].content);
                        }
                    }
                    nodes[i].content = merged_content;
                    nodes[i].token_count = Some(nodes[i].token_count.unwrap_or(0));
                }

                // Mark children for removal
                for &child_idx in &children_indices {
                    keep[child_idx] = false;
                }
            }
        }

        // Ensure each parent keeps at least one child
        Self::ensure_min_children(nodes, &mut keep);

        keep
    }

    /// Ensure each parent keeps at least one direct child.
    fn ensure_min_children(nodes: &[RawNode], keep: &mut [bool]) {
        for i in 0..nodes.len() {
            let children = Self::find_direct_children_indices(i, nodes);

            if !children.is_empty() {
                let has_kept_child = children.iter().any(|&c| keep[c]);

                if !has_kept_child {
                    // Keep the child with the most content
                    let best_child = children
                        .iter()
                        .max_by_key(|&&c| nodes[c].total_token_count.unwrap_or(0))
                        .copied();

                    if let Some(idx) = best_child {
                        keep[idx] = true;
                    }
                }
            }
        }
    }

    /// Build tree from raw nodes.
    fn build_tree(&self, raw_nodes: Vec<RawNode>, ctx: &mut CompileContext) -> DocumentTree {
        let root_title = ctx.name.clone();
        let root_content = String::new();

        let mut tree = DocumentTree::new(&root_title, &root_content);

        // Stack to track parent nodes at each level
        let mut level_stack: Vec<Option<NodeId>> = vec![Some(tree.root())];

        for raw in raw_nodes {
            let level = raw.level;

            // Ensure stack has enough slots
            while level_stack.len() <= level {
                level_stack.push(None);
            }

            // Find parent: closest ancestor with a lower level
            let parent_id = (0..level)
                .rev()
                .find_map(|l| level_stack.get(l).copied().flatten())
                .unwrap_or(tree.root());

            // Create the node
            let content = if raw.content.is_empty() {
                ""
            } else {
                &raw.content
            };
            let node_id = tree.add_child(parent_id, &raw.title, content);

            // Set line indices
            tree.set_line_indices(node_id, raw.line_start, raw.line_end);

            // Set page boundaries if available
            if let Some(page) = raw.page {
                tree.set_page_boundaries(node_id, page, page);
            }

            // Set token count if available
            if let Some(count) = raw.token_count {
                if count > 0 {
                    tree.set_token_count(node_id, count);
                }
            }

            // Update the stack for this level
            if level < level_stack.len() {
                level_stack[level] = Some(node_id);
            }

            // Clear deeper levels
            for i in (level + 1)..level_stack.len() {
                level_stack[i] = None;
            }
        }

        tree
    }

    /// Assign unique node IDs (DFS traversal).
    fn assign_node_ids(&self, tree: &mut DocumentTree) {
        let mut counter: usize = 0;
        self.assign_recursive(tree, tree.root(), &mut counter);
    }

    fn assign_recursive(&self, tree: &mut DocumentTree, node_id: NodeId, counter: &mut usize) {
        *counter += 1;
        let id_str = format!("{:04}", counter);
        tree.set_node_id(node_id, &id_str);

        let children = tree.children(node_id);
        for child_id in children {
            self.assign_recursive(tree, child_id, counter);
        }
    }
}

impl Default for BuildPass {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl CompilePass for BuildPass {
    fn name(&self) -> &'static str {
        "build"
    }

    fn depends_on(&self) -> Vec<&'static str> {
        vec!["parse"]
    }

    async fn execute(&mut self, ctx: &mut CompileContext) -> Result<PassResult> {
        let start = Instant::now();

        // Take raw nodes from context
        let mut raw_nodes = std::mem::take(&mut ctx.raw_nodes);

        if raw_nodes.is_empty() {
            info!("[build] No raw nodes, skipping");
            return Ok(PassResult::success("build"));
        }

        info!(
            "[build] Starting: {} raw nodes, thinning={}",
            raw_nodes.len(),
            ctx.options.thinning.enabled
        );

        // Step 1: Calculate total tokens
        Self::calculate_total_tokens(&mut raw_nodes);
        debug!(
            "[build] Calculated total tokens for {} nodes",
            raw_nodes.len()
        );

        // Step 2: Apply thinning if enabled
        let _original_count = raw_nodes.len();
        let keep = Self::apply_thinning(&mut raw_nodes, &ctx.options.thinning);

        let nodes_before_merge = raw_nodes.len();
        raw_nodes = raw_nodes
            .into_iter()
            .zip(keep)
            .filter_map(|(node, k)| if k { Some(node) } else { None })
            .collect();

        let skipped = nodes_before_merge - raw_nodes.len();
        ctx.metrics.nodes_skipped += skipped;
        if skipped > 0 {
            debug!(
                "[build] Thinning removed {} nodes ({} → {})",
                skipped,
                nodes_before_merge,
                raw_nodes.len()
            );
        }

        // Step 3: Build tree
        let mut tree = self.build_tree(raw_nodes, ctx);

        // Step 4: Assign node IDs if configured
        if ctx.options.generate_ids {
            self.assign_node_ids(&mut tree);
        }

        let node_count = tree.node_count();

        // Store tree in context
        ctx.tree = Some(tree);

        let duration = start.elapsed().as_millis() as u64;
        ctx.metrics.record_build(duration);

        info!(
            "[build] Complete: {} nodes (skipped {} via thinning) in {}ms",
            node_count, skipped, duration
        );

        let mut stage_result = PassResult::success("build");
        stage_result.duration_ms = duration;
        stage_result.metadata.insert(
            "node_count".to_string(),
            serde_json::json!(ctx.tree.as_ref().map(|t| t.node_count()).unwrap_or(0)),
        );
        stage_result
            .metadata
            .insert("nodes_skipped".to_string(), serde_json::json!(skipped));

        Ok(stage_result)
    }
}
