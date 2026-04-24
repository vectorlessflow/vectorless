// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Overlap Pass — detects content overlap between leaf nodes.
//!
//! Computes pairwise Jaccard similarity on leaf node content to identify
//! duplicate or near-duplicate sections. The Agent can skip overlapping nodes.

use std::collections::HashSet;
use std::time::Instant;
use tracing::{debug, info, warn};

use vectorless_document::{ContentOverlapMap, OverlapEntry, OverlapType, NodeId};
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
        let words_a: HashSet<&str> = a.to_lowercase().split_whitespace().collect();
        let words_b: HashSet<&str> = b.to_lowercase().split_whitespace().collect();

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

        ctx.content_overlap = Some(overlap_map);

        let mut result = PassResult::success("overlap");
        result.duration_ms = duration;
        result.metadata.insert(
            "overlaps".to_string(),
            serde_json::json!(overlap_count),
        );
        result.metadata.insert(
            "comparisons".to_string(),
            serde_json::json!(comparisons),
        );

        Ok(result)
    }
}
