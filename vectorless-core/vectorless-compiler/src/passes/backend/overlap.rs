// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Overlap Pass — detects content overlap between leaf nodes.
//!
//! Computes pairwise Jaccard similarity on leaf node content to identify
//! duplicate or near-duplicate sections. The Agent can skip overlapping nodes.

use std::collections::HashSet;
use std::time::Instant;
use tracing::{debug, info, warn};

use vectorless_document::{ContentOverlapMap, NodeId, OverlapEntry, OverlapType};
use vectorless_error::Result;

use crate::passes::async_trait;
use crate::passes::{AccessPattern, CompilePass, PassResult};
use crate::pipeline::CompileContext;

/// Jaccard similarity threshold for overlap detection.
const SIMILARITY_THRESHOLD: f64 = 0.8;

/// Overlap Pass — builds content overlap map.
pub struct OverlapPass;

impl OverlapPass {
    /// Create a new overlap pass.
    pub fn new() -> Self {
        Self
    }

    /// Compute Jaccard similarity between two strings (word-level).
    fn jaccard(a: &str, b: &str) -> f64 {
        let a_lower = a.to_lowercase();
        let b_lower = b.to_lowercase();
        let words_a: HashSet<&str> = a_lower.split_whitespace().collect();
        let words_b: HashSet<&str> = b_lower.split_whitespace().collect();

        if words_a.is_empty() && words_b.is_empty() {
            return 1.0;
        }
        if words_a.is_empty() || words_b.is_empty() {
            return 0.0;
        }

        let intersection = words_a.intersection(&words_b).count() as f64;
        let union = words_a.union(&words_b).count() as f64;
        intersection / union
    }

    /// Classify overlap type based on similarity and content length ratio.
    fn classify_overlap(similarity: f64, len_a: usize, len_b: usize) -> OverlapType {
        if similarity >= 0.9 {
            OverlapType::Duplicate
        } else {
            let ratio = if len_a > 0 && len_b > 0 {
                (len_a.min(len_b) as f64) / (len_a.max(len_b) as f64)
            } else {
                0.0
            };
            if ratio < 0.5 {
                OverlapType::Summary
            } else {
                OverlapType::Subset
            }
        }
    }
}

impl Default for OverlapPass {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl CompilePass for OverlapPass {
    fn name(&self) -> &'static str {
        "overlap"
    }

    fn depends_on(&self) -> Vec<&'static str> {
        vec!["build"]
    }

    fn is_optional(&self) -> bool {
        true
    }

    fn access_pattern(&self) -> AccessPattern {
        AccessPattern {
            reads_tree: true,
            writes_content_overlap: true,
            ..Default::default()
        }
    }

    async fn execute(&mut self, ctx: &mut CompileContext) -> Result<PassResult> {
        let start = Instant::now();

        let tree = match ctx.tree.as_ref() {
            Some(t) => t,
            None => {
                warn!("[overlap] No tree, cannot compute overlaps");
                return Ok(PassResult::failure("overlap", "Tree not built"));
            }
        };

        let leaves: Vec<NodeId> = tree.leaves();
        let leaf_count = leaves.len();

        info!("[overlap] Computing overlaps for {} leaf nodes", leaf_count);

        // Skip if too few leaves (no pairs to compare)
        if leaf_count < 2 {
            debug!("[overlap] Fewer than 2 leaves, no overlaps possible");
            ctx.content_overlap = Some(ContentOverlapMap::new());
            let mut result = PassResult::success("overlap");
            result.duration_ms = start.elapsed().as_millis() as u64;
            return Ok(result);
        }

        let mut overlap_map = ContentOverlapMap::new();
        let mut comparisons = 0usize;

        // Pairwise Jaccard on leaf content
        for i in 0..leaf_count {
            let node_a = leaves[i];
            let content_a = match tree.get(node_a) {
                Some(n) => &n.content,
                None => continue,
            };

            // Skip very short content (< 50 chars)
            if content_a.len() < 50 {
                continue;
            }

            for j in (i + 1)..leaf_count {
                let node_b = leaves[j];
                let content_b = match tree.get(node_b) {
                    Some(n) => &n.content,
                    None => continue,
                };

                if content_b.len() < 50 {
                    continue;
                }

                comparisons += 1;
                let similarity = Self::jaccard(content_a, content_b);

                if similarity >= SIMILARITY_THRESHOLD {
                    overlap_map.add(OverlapEntry {
                        node_a,
                        node_b,
                        similarity,
                        overlap_type: Self::classify_overlap(
                            similarity,
                            content_a.len(),
                            content_b.len(),
                        ),
                    });
                }
            }
        }

        let overlap_count = overlap_map.overlap_count();
        let duration = start.elapsed().as_millis() as u64;

        info!(
            "[overlap] Complete: {} overlaps from {} comparisons in {}ms",
            overlap_count, comparisons, duration,
        );

        ctx.metrics.record_overlap(duration, overlap_count);

        ctx.content_overlap = Some(overlap_map);

        let mut result = PassResult::success("overlap");
        result.duration_ms = duration;
        result
            .metadata
            .insert("overlaps".to_string(), serde_json::json!(overlap_count));
        result
            .metadata
            .insert("comparisons".to_string(), serde_json::json!(comparisons));

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_jaccard_identical() {
        let sim = OverlapPass::jaccard("hello world foo bar", "hello world foo bar");
        assert!((sim - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_jaccard_no_overlap() {
        let sim = OverlapPass::jaccard("alpha beta gamma", "delta epsilon zeta");
        assert!((sim - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_jaccard_partial() {
        let sim = OverlapPass::jaccard("alpha beta gamma", "beta gamma delta");
        // intersection: beta, gamma (2), union: alpha, beta, gamma, delta (4) = 0.5
        assert!((sim - 0.5).abs() < 1e-10);
    }

    #[test]
    fn test_jaccard_empty() {
        assert!((OverlapPass::jaccard("", "") - 1.0).abs() < f64::EPSILON);
        assert!((OverlapPass::jaccard("content", "") - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_jaccard_case_insensitive() {
        let sim = OverlapPass::jaccard("Hello World", "hello world");
        assert!((sim - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_classify_overlap_duplicate() {
        assert_eq!(
            OverlapPass::classify_overlap(0.95, 100, 100),
            OverlapType::Duplicate
        );
    }

    #[test]
    fn test_classify_overlap_subset() {
        // 0.85 similarity with similar lengths → Subset
        assert_eq!(
            OverlapPass::classify_overlap(0.85, 100, 90),
            OverlapType::Subset
        );
    }

    #[test]
    fn test_classify_overlap_summary() {
        // 0.85 similarity with very different lengths → Summary
        assert_eq!(
            OverlapPass::classify_overlap(0.85, 100, 30),
            OverlapType::Summary
        );
    }

    #[test]
    fn test_stage_config() {
        let pass = OverlapPass::new();
        assert_eq!(pass.name(), "overlap");
        assert!(pass.is_optional());
        assert_eq!(pass.depends_on(), vec!["build"]);

        let ap = pass.access_pattern();
        assert!(ap.reads_tree);
        assert!(ap.writes_content_overlap);
        assert!(!ap.writes_tree);
    }

    #[tokio::test]
    async fn test_execute_no_tree() {
        let mut ctx = CompileContext::new(
            crate::pipeline::CompilerInput::content("test"),
            crate::config::PipelineOptions::default(),
        );
        ctx.tree = None;

        let mut pass = OverlapPass::new();
        let result = pass.execute(&mut ctx).await.unwrap();
        assert!(!result.success);
        assert!(ctx.content_overlap.is_none());
    }

    #[tokio::test]
    async fn test_execute_single_leaf() {
        let tree = vectorless_document::DocumentTree::new("Root", "single leaf content");
        let mut ctx = CompileContext::new(
            crate::pipeline::CompilerInput::content("test"),
            crate::config::PipelineOptions::default(),
        );
        ctx.tree = Some(tree);

        let mut pass = OverlapPass::new();
        let result = pass.execute(&mut ctx).await;

        assert!(result.is_ok());
        assert!(result.unwrap().success);
        let map = ctx.content_overlap.unwrap();
        assert_eq!(map.overlap_count(), 0);
    }

    #[tokio::test]
    async fn test_execute_with_duplicates() {
        let mut tree = vectorless_document::DocumentTree::new("Root", "");
        let root = tree.root();

        // Two leaf nodes with identical content (long enough to pass the 50-char threshold)
        let long_content = "This is a sufficiently long piece of content that should pass the minimum length threshold for overlap detection in the system.".to_string();
        tree.add_child(root, "Section A", &long_content);
        tree.add_child(root, "Section B", &long_content);

        let mut ctx = CompileContext::new(
            crate::pipeline::CompilerInput::content("test"),
            crate::config::PipelineOptions::default(),
        );
        ctx.tree = Some(tree);

        let mut pass = OverlapPass::new();
        let result = pass.execute(&mut ctx).await;

        assert!(result.is_ok());
        assert!(result.unwrap().success);

        let map = ctx.content_overlap.unwrap();
        assert_eq!(map.overlap_count(), 1);
        assert_eq!(map.overlaps[0].overlap_type, OverlapType::Duplicate);
    }

    #[tokio::test]
    async fn test_execute_no_overlap() {
        let mut tree = vectorless_document::DocumentTree::new("Root", "");
        let root = tree.root();

        // Two leaf nodes with completely different content
        tree.add_child(root, "Section A",
            "Alpha beta gamma delta epsilon zeta eta theta iota kappa lambda mu nu xi omicron pi rho sigma tau upsilon phi chi psi omega");
        tree.add_child(root, "Section B",
            "Apple banana cherry date elderberry fig grape honeydew kiwi lemon mango nectarine orange papaya quince raspberry strawberry tangerine");

        let mut ctx = CompileContext::new(
            crate::pipeline::CompilerInput::content("test"),
            crate::config::PipelineOptions::default(),
        );
        ctx.tree = Some(tree);

        let mut pass = OverlapPass::new();
        let result = pass.execute(&mut ctx).await;

        assert!(result.is_ok());
        assert!(result.unwrap().success);

        let map = ctx.content_overlap.unwrap();
        assert_eq!(map.overlap_count(), 0);
    }
}
