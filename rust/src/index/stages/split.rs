// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Split stage - Break large leaf nodes into smaller ones.

use std::time::Instant;
use tracing::info;

use crate::document::{DocumentTree, NodeId};
use crate::error::Result;
use crate::utils::estimate_tokens;

use super::{AccessPattern, IndexStage, StageResult, async_trait};
use crate::index::config::SplitConfig;
use crate::index::pipeline::IndexContext;

/// Split stage — breaks oversized leaf nodes into smaller children.
///
/// When a leaf node exceeds the token limit, the stage searches for natural
/// split points (headings `\n#`, paragraph boundaries `\n\n`) and creates
/// child nodes from the resulting chunks.
///
/// This stage runs after validate (priority 22) at priority 25.
pub struct SplitStage;

impl SplitStage {
    /// Create a new split stage.
    pub fn new() -> Self {
        Self
    }

    /// Find natural split points in content.
    ///
    /// Returns byte offsets where the content can be split.
    /// Prioritizes heading boundaries (`\n#`), then paragraph breaks (`\n\n`).
    fn find_split_points(content: &str, max_tokens: usize) -> Vec<usize> {
        let total_tokens = estimate_tokens(content);
        if total_tokens <= max_tokens {
            return Vec::new();
        }

        // Estimate how many parts we need
        let estimated_parts = (total_tokens + max_tokens - 1) / max_tokens;
        let target_size = content.len() / estimated_parts.max(1);

        let mut points = Vec::new();

        // First pass: find heading boundaries
        let mut last_split = 0;
        for (i, line) in content.lines().enumerate() {
            let byte_offset = line.as_ptr() as usize - content.as_ptr() as usize;
            if i > 0 && line.starts_with('#') && byte_offset > last_split {
                let chunk_tokens = estimate_tokens(&content[last_split..byte_offset]);
                if chunk_tokens >= max_tokens / 2 {
                    points.push(byte_offset);
                    last_split = byte_offset;
                }
            }
        }

        // If heading splits are sufficient, return them
        if !points.is_empty() {
            let approx_size = content.len() / (points.len() + 1);
            if approx_size <= target_size * 2 {
                return points;
            }
        }

        // Second pass: use paragraph boundaries
        points.clear();
        let mut pos = 0;
        for paragraph in content.split("\n\n") {
            let para_end = pos + paragraph.len();
            if para_end > 0 && pos > 0 {
                let chunk_tokens =
                    estimate_tokens(&content[points.last().copied().unwrap_or(0)..pos]);
                if chunk_tokens >= max_tokens / 2 {
                    points.push(pos);
                }
            }
            pos = para_end + 2; // skip "\n\n"
        }

        // If still not enough split points, use approximate byte boundaries
        if points.is_empty() {
            let bytes_per_token = content.len().max(1) / total_tokens.max(1);
            let target_bytes = max_tokens * bytes_per_token;

            let mut offset = target_bytes;
            while offset < content.len() {
                // Find the nearest newline
                if let Some(nl_pos) = content[offset..].find('\n') {
                    points.push(offset + nl_pos);
                } else {
                    break;
                }
                offset += target_bytes;
            }
        }

        points
    }

    /// Split a single leaf node into children.
    ///
    /// Returns the number of new children created.
    fn split_leaf(tree: &mut DocumentTree, leaf_id: NodeId, max_tokens: usize) -> usize {
        let content = match tree.get(leaf_id) {
            Some(node) => node.content.clone(),
            None => return 0,
        };

        let split_points = Self::find_split_points(&content, max_tokens);
        if split_points.is_empty() {
            return 0;
        }

        // Extract title for child naming
        let parent_title = tree
            .get(leaf_id)
            .map(|n| n.title.clone())
            .unwrap_or_default();

        // Create chunks from split points
        let mut chunks: Vec<&str> = Vec::new();
        let mut prev = 0;
        for &point in &split_points {
            if point > prev {
                chunks.push(&content[prev..point]);
            }
            prev = point;
        }
        if prev < content.len() {
            chunks.push(&content[prev..]);
        }

        let child_count = chunks.len();
        for (i, chunk) in chunks.into_iter().enumerate() {
            let chunk_trimmed = chunk.trim();
            if chunk_trimmed.is_empty() {
                continue;
            }

            // Try to extract a title from the first line
            let title = if chunk_trimmed.starts_with('#') {
                chunk_trimmed
                    .lines()
                    .next()
                    .unwrap_or("")
                    .trim_start_matches('#')
                    .trim()
                    .to_string()
            } else {
                format!("{} (part {})", parent_title, i + 1)
            };

            let child_id = tree.add_child(leaf_id, &title, chunk_trimmed);
            let token_count = estimate_tokens(chunk_trimmed);
            tree.set_token_count(child_id, token_count);
        }

        // Clear parent's content (moved to children)
        tree.set_content(leaf_id, "");
        tree.set_token_count(leaf_id, 0);

        child_count
    }

    /// Process all oversized leaf nodes in the tree.
    fn split_tree(tree: &mut DocumentTree, config: &SplitConfig) -> usize {
        if !config.enabled {
            return 0;
        }

        // Collect leaves first to avoid borrow issues
        let leaves: Vec<NodeId> = tree.leaves();
        let mut total_split = 0;

        for leaf_id in leaves {
            // Check if this leaf exceeds the token limit
            let token_count = tree.get(leaf_id).and_then(|n| n.token_count).unwrap_or(0);

            // Use estimated tokens if no count set
            let tokens = if token_count > 0 {
                token_count
            } else {
                tree.get(leaf_id)
                    .map(|n| estimate_tokens(&n.content))
                    .unwrap_or(0)
            };

            if tokens > config.max_tokens_per_node {
                let split_count = Self::split_leaf(tree, leaf_id, config.max_tokens_per_node);
                if split_count > 0 {
                    total_split += 1;
                }
            }
        }

        total_split
    }
}

impl Default for SplitStage {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl IndexStage for SplitStage {
    fn name(&self) -> &'static str {
        "split"
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
            writes_tree: true,
            writes_reasoning_index: false,
            writes_navigation_index: false,
            writes_description: false,
        }
    }

    async fn execute(&mut self, ctx: &mut IndexContext) -> Result<StageResult> {
        let start = Instant::now();

        let tree = match ctx.tree.as_mut() {
            Some(t) => t,
            None => {
                return Ok(StageResult::success("split"));
            }
        };

        let config = &ctx.options.split;
        if !config.enabled {
            return Ok(StageResult::success("split"));
        }

        let node_count_before = tree.node_count();
        let split_count = Self::split_tree(tree, config);
        let node_count_after = tree.node_count();

        let duration = start.elapsed().as_millis() as u64;
        ctx.metrics.record_split(duration);
        ctx.metrics.nodes_merged += split_count;

        info!(
            "Split {} oversized nodes ({} → {} total nodes) in {}ms",
            split_count, node_count_before, node_count_after, duration
        );

        let mut stage_result = StageResult::success("split");
        stage_result.duration_ms = duration;
        stage_result
            .metadata
            .insert("nodes_split".to_string(), serde_json::json!(split_count));
        stage_result.metadata.insert(
            "node_count_before".to_string(),
            serde_json::json!(node_count_before),
        );
        stage_result.metadata.insert(
            "node_count_after".to_string(),
            serde_json::json!(node_count_after),
        );

        Ok(stage_result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_split_points_small_content() {
        let content = "Hello world";
        let points = SplitStage::find_split_points(content, 8000);
        assert!(points.is_empty());
    }

    #[test]
    fn test_find_split_points_heading_boundaries() {
        let mut content = String::from("Introduction text that is long enough. ");
        // Pad to exceed token limit
        for _ in 0..500 {
            content.push_str("This is some content. ");
        }
        content.push_str("\n## Section One\n");
        for _ in 0..500 {
            content.push_str("More content here. ");
        }
        content.push_str("\n## Section Two\n");
        for _ in 0..500 {
            content.push_str("Final content. ");
        }

        let points = SplitStage::find_split_points(&content, 200);
        assert!(!points.is_empty());
    }

    #[test]
    fn test_find_split_points_paragraph_boundaries() {
        let mut content = String::new();
        for i in 0..10 {
            for _ in 0..100 {
                content.push_str(&format!("Paragraph {} content. ", i));
            }
            content.push_str("\n\n");
        }

        let points = SplitStage::find_split_points(&content, 200);
        assert!(!points.is_empty());
    }

    #[test]
    fn test_split_tree_disabled() {
        let mut tree = DocumentTree::new("Root", "");
        let child = tree.add_child(
            tree.root(),
            "Big",
            "Very long content here with lots of text that would normally exceed limits",
        );
        tree.set_token_count(child, 15000);

        let config = SplitConfig::disabled();
        let count = SplitStage::split_tree(&mut tree, &config);
        assert_eq!(count, 0);
    }
}
