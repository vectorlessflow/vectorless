// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Chain Pass — builds reasoning chain index from in-document references.
//!
//! Analyzes TreeNode.references to find premise→conclusion relationships
//! between sections. No LLM calls — uses reference types and tree structure.

use std::time::Instant;
use tracing::{info, warn};

use vectorless_document::{ChainIndex, ChainType, ReasoningChain, RefType};
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
        ref_type: RefType,
        source_depth: usize,
        target_depth: usize,
    ) -> ChainType {
        match ref_type {
            RefType::Section => {
                if target_depth > source_depth {
                    ChainType::Elaboration
                } else {
                    ChainType::Supporting
                }
            }
            RefType::Appendix | RefType::Table | RefType::Figure | RefType::Equation => {
                ChainType::Supporting
            }
            RefType::Footnote => ChainType::Elaboration,
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
                    reference.ref_type,
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

        ctx.metrics.record_chain(duration, chain_count);

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

#[cfg(test)]
mod tests {
    use super::*;
    use vectorless_document::{NodeReference, RefType};

    fn build_test_tree_with_refs() -> vectorless_document::DocumentTree {
        let mut tree = vectorless_document::DocumentTree::new("Root", "root content");
        let root = tree.root();

        let sec1 = tree.add_child(root, "Introduction", "See Section 2 for details");
        let sec2 = tree.add_child(root, "Methods", "As shown in Table 1");
        let appendix = tree.add_child(root, "Appendix A", "Supporting data");

        // Add references: sec1 → sec2 (Section ref), sec2 → appendix (Appendix ref)
        if let Some(n) = tree.get_mut(sec1) {
            n.references = vec![NodeReference::resolved(
                "see Section 2".to_string(),
                "2".to_string(),
                RefType::Section,
                4,
                sec2,
                1.0,
            )];
        }
        if let Some(n) = tree.get_mut(sec2) {
            n.references = vec![NodeReference::resolved(
                "Appendix A".to_string(),
                "A".to_string(),
                RefType::Appendix,
                12,
                appendix,
                1.0,
            )];
        }

        tree
    }

    #[test]
    fn test_stage_config() {
        let pass = ChainPass::new();
        assert_eq!(pass.name(), "chain");
        assert!(pass.is_optional());
        assert_eq!(pass.depends_on(), vec!["enrich"]);

        let ap = pass.access_pattern();
        assert!(ap.reads_tree);
        assert!(ap.writes_chain_index);
        assert!(!ap.writes_tree);
    }

    #[test]
    fn test_classify_chain_section_elaboration() {
        assert_eq!(ChainPass::classify_chain(RefType::Section, 0, 1), ChainType::Elaboration);
    }

    #[test]
    fn test_classify_chain_section_supporting() {
        assert_eq!(ChainPass::classify_chain(RefType::Section, 1, 0), ChainType::Supporting);
    }

    #[test]
    fn test_classify_chain_appendix() {
        assert_eq!(ChainPass::classify_chain(RefType::Appendix, 0, 1), ChainType::Supporting);
    }

    #[test]
    fn test_classify_chain_table_figure() {
        assert_eq!(ChainPass::classify_chain(RefType::Table, 0, 1), ChainType::Supporting);
        assert_eq!(ChainPass::classify_chain(RefType::Figure, 0, 1), ChainType::Supporting);
        assert_eq!(ChainPass::classify_chain(RefType::Equation, 0, 1), ChainType::Supporting);
    }

    #[test]
    fn test_classify_chain_footnote() {
        assert_eq!(ChainPass::classify_chain(RefType::Footnote, 0, 2), ChainType::Elaboration);
    }

    #[test]
    fn test_classify_chain_unknown() {
        assert_eq!(ChainPass::classify_chain(RefType::Unknown, 0, 0), ChainType::Supporting);
    }

    #[tokio::test]
    async fn test_execute_end_to_end() {
        let tree = build_test_tree_with_refs();

        let mut ctx = CompileContext::new(
            crate::pipeline::CompilerInput::content("test"),
            crate::config::PipelineOptions::default(),
        );
        ctx.tree = Some(tree);

        let mut pass = ChainPass::new();
        let result = pass.execute(&mut ctx).await;

        assert!(result.is_ok());
        let pass_result = result.unwrap();
        assert!(pass_result.success);

        let index = ctx.chain_index.unwrap();
        assert_eq!(index.chain_count(), 2); // sec1→sec2, sec2→appendix
        assert!(index.node_count() >= 2);
    }

    #[tokio::test]
    async fn test_execute_no_tree() {
        let mut ctx = CompileContext::new(
            crate::pipeline::CompilerInput::content("test"),
            crate::config::PipelineOptions::default(),
        );
        ctx.tree = None;

        let mut pass = ChainPass::new();
        let result = pass.execute(&mut ctx).await.unwrap();
        assert!(!result.success);
        assert!(ctx.chain_index.is_none());
    }

    #[tokio::test]
    async fn test_execute_no_references() {
        let tree = vectorless_document::DocumentTree::new("Root", "no references");
        let mut ctx = CompileContext::new(
            crate::pipeline::CompilerInput::content("test"),
            crate::config::PipelineOptions::default(),
        );
        ctx.tree = Some(tree);

        let mut pass = ChainPass::new();
        let result = pass.execute(&mut ctx).await;

        assert!(result.is_ok());
        assert!(result.unwrap().success);

        let index = ctx.chain_index.unwrap();
        assert_eq!(index.chain_count(), 0);
    }
}
