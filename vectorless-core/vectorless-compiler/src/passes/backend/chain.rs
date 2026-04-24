// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Chain Pass — builds reasoning chain index from in-document references.
//!
//! Analyzes TreeNode.references to find premise→conclusion relationships
//! between sections. No LLM calls — uses reference types and tree structure.

use std::time::Instant;
use tracing::{debug, info, warn};

use vectorless_document::{ChainIndex, ChainType, ReasoningChain, NodeId};
use vectorless_error::Result;

use crate::passes::async_trait;
use crate::passes::{AccessPattern, CompilePass, PassResult};
use crate::pipeline::CompileContext;

/// Chain Pass — builds reasoning chain index.
///
/// Uses the references field on TreeNode (populated by EnrichPass) to
/// identify logical connections between sections.
pub struct ChainPass;

impl ChainPass {
    /// Create a new chain pass.
    pub fn new() -> Self {
        Self
    }

    /// Determine chain type from the reference structure.
    fn classify_chain(
        ref_type: &str,
        source_depth: usize,
        target_depth: usize,
    ) -> ChainType {
        match ref_type {
            "Section" | "section" => {
                if target_depth > source_depth {
                    ChainType::Elaboration
                } else {
                    ChainType::Supporting
                }
            }
            "Appendix" | "appendix" => ChainType::Supporting,
            "Table" | "Figure" | "Equation" | "table" | "figure" | "equation" => {
                ChainType::Supporting
            }
            "Footnote" | "footnote" => ChainType::Elaboration,
            _ => ChainType::Supporting,
        }
    }
}

impl Default for ChainPass {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl CompilePass for ChainPass {
    fn name(&self) -> &'static str {
        "chain"
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
            writes_chain_index: true,
            ..Default::default()
        }
    }

    async fn execute(&mut self, ctx: &mut CompileContext) -> Result<PassResult> {
        let start = Instant::now();

        let tree = match ctx.tree.as_ref() {
            Some(t) => t,
            None => {
                warn!("[chain] No tree, cannot build chain index");
                return Ok(PassResult::failure("chain", "Tree not built"));
            }
        };

        let all_nodes = tree.traverse();
        let mut index = ChainIndex::new();

        for &node_id in &all_nodes {
            let node = match tree.get(node_id) {
                Some(n) => n,
                None => continue,
            };

            if node.references.is_empty() {
                continue;
            }

            for reference in &node.references {
                // Only process references with resolved targets
                let target_id = match reference.target_node {
                    Some(id) => id,
                    None => continue,
                };

                let chain_type = Self::classify_chain(
                    &reference.ref_text,
                    node.depth,
                    tree.get(target_id).map(|n| n.depth).unwrap_or(0),
                );

                let target_title = tree
                    .get(target_id)
                    .map(|n| n.title.as_str())
                    .unwrap_or("unknown");

                index.add_chain(ReasoningChain {
                    premises: vec![node_id],
                    conclusions: vec![target_id],
                    chain_type,
                    summary: format!(
                        "{} references {} ({})",
                        node.title, target_title, reference.ref_text
                    ),
                });
            }
        }

        let chain_count = index.chain_count();
        let node_count = index.node_count();
        let duration = start.elapsed().as_millis() as u64;

        info!(
            "[chain] Complete: {} chains involving {} nodes in {}ms",
            chain_count, node_count, duration,
        );

        ctx.chain_index = Some(index);

        let mut result = PassResult::success("chain");
        result.duration_ms = duration;
        result.metadata.insert(
            "chains".to_string(),
            serde_json::json!(chain_count),
        );
        result.metadata.insert(
            "nodes".to_string(),
            serde_json::json!(node_count),
        );

        Ok(result)
    }
}
