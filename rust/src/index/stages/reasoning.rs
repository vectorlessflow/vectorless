// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Reasoning Index Stage - Build pre-computed reasoning index.
//!
//! This stage runs after EnrichStage (which generates descriptions and
//! calculates metadata) and before OptimizeStage. It builds a
//! [`ReasoningIndex`] from the document tree's TOC, summaries, and keywords.

use std::time::Instant;
use tracing::info;

use crate::document::{
    NodeId, ReasoningIndexBuilder, ReasoningIndexConfig, SectionSummary,
    SummaryShortcut, TopicEntry,
};
use crate::error::Result;
use crate::retrieval::search::extract_keywords;

use super::async_trait;
use super::{AccessPattern, IndexStage, StageResult};
use crate::index::pipeline::IndexContext;

/// Reasoning Index Stage - builds a pre-computed reasoning index from the document tree.
///
/// This stage creates a [`ReasoningIndex`] containing:
/// - Topic-to-path mappings from titles and summaries
/// - Summary shortcuts for high-frequency "overview" queries
/// - Section map for fast ToC lookup
pub struct ReasoningIndexStage {
    config: ReasoningIndexConfig,
}

impl ReasoningIndexStage {
    /// Create a new reasoning index stage with default config.
    pub fn new() -> Self {
        Self {
            config: ReasoningIndexConfig::default(),
        }
    }

    /// Create with custom config.
    pub fn with_config(config: ReasoningIndexConfig) -> Self {
        Self { config }
    }

    /// Extract keywords from a text, filtering by minimum length.
    fn extract_node_keywords(text: &str, min_length: usize) -> Vec<String> {
        extract_keywords(text)
            .into_iter()
            .filter(|k: &String| k.len() >= min_length)
            .collect()
    }

    /// Build the topic-to-path mapping by extracting keywords from all nodes.
    fn build_topic_paths(
        tree: &crate::document::DocumentTree,
        config: &ReasoningIndexConfig,
    ) -> (std::collections::HashMap<String, Vec<TopicEntry>>, usize) {
        let mut keyword_nodes: std::collections::HashMap<String, Vec<(NodeId, f32, usize)>> =
            std::collections::HashMap::new();

        // Walk all nodes and extract keywords from title + summary
        for node_id in tree.traverse() {
            if let Some(node) = tree.get(node_id) {
                let title_keywords = Self::extract_node_keywords(&node.title, config.min_keyword_length);
                let summary_keywords = Self::extract_node_keywords(&node.summary, config.min_keyword_length);
                let content_keywords = if node.summary.is_empty() {
                    // Fallback: extract from content if no summary
                    let content_sample: String = node.content.chars().take(500).collect();
                    Self::extract_node_keywords(&content_sample, config.min_keyword_length)
                } else {
                    Vec::new()
                };

                // Title keywords get higher weight (2.0), summary (1.5), content (1.0)
                for kw in &title_keywords {
                    keyword_nodes
                        .entry(kw.clone())
                        .or_default()
                        .push((node_id, 2.0, node.depth));
                }
                for kw in &summary_keywords {
                    keyword_nodes
                        .entry(kw.clone())
                        .or_default()
                        .push((node_id, 1.5, node.depth));
                }
                for kw in &content_keywords {
                    keyword_nodes
                        .entry(kw.clone())
                        .or_default()
                        .push((node_id, 1.0, node.depth));
                }
            }
        }

        // Sort by keyword frequency (most common first) and trim to max_keyword_entries
        let mut sorted_keywords: Vec<_> = keyword_nodes.into_iter().collect();
        sorted_keywords.sort_by(|a, b| b.1.len().cmp(&a.1.len()));
        sorted_keywords.truncate(config.max_keyword_entries);

        let keyword_count = sorted_keywords.len();

        // Build topic_paths: merge duplicate (keyword, node) pairs
        let mut topic_paths: std::collections::HashMap<String, Vec<TopicEntry>> =
            std::collections::HashMap::new();

        for (keyword, entries) in sorted_keywords {
            // Merge duplicate node entries by summing weights
            let mut merged: std::collections::HashMap<NodeId, (f32, usize)> =
                std::collections::HashMap::new();
            for (node_id, weight, depth) in entries {
                let entry = merged.entry(node_id).or_insert((0.0, depth));
                entry.0 += weight;
            }

            // Normalize weights to 0.0-1.0 range
            let max_weight = merged.values().map(|(w, _)| *w).fold(0.0_f32, f32::max);
            let scale = if max_weight > 0.0 { 1.0 / max_weight } else { 1.0 };

            let mut topic_entries: Vec<TopicEntry> = merged
                .into_iter()
                .map(|(node_id, (weight, depth))| TopicEntry {
                    node_id,
                    weight: weight * scale,
                    depth,
                })
                .collect();

            topic_entries.sort_by(|a, b| {
                b.weight
                    .partial_cmp(&a.weight)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            topic_entries.truncate(config.max_topic_entries);

            topic_paths.insert(keyword, topic_entries);
        }

        (topic_paths, keyword_count)
    }

    /// Build section map from depth-1 nodes.
    fn build_section_map(tree: &crate::document::DocumentTree) -> std::collections::HashMap<String, NodeId> {
        let mut section_map = std::collections::HashMap::new();
        let root = tree.root();
        for child_id in tree.children(root) {
            if let Some(node) = tree.get(child_id) {
                section_map.insert(node.title.to_lowercase(), child_id);
                // Also index by structure index (e.g. "1", "2", "3")
                if !node.structure.is_empty() {
                    section_map.insert(node.structure.clone(), child_id);
                }
            }
        }
        section_map
    }

    /// Build summary shortcut from root and depth-1 nodes.
    fn build_summary_shortcut(
        tree: &crate::document::DocumentTree,
    ) -> Option<SummaryShortcut> {
        let root = tree.root();
        let root_node = tree.get(root)?;

        // Collect document summary from root
        let document_summary = if !root_node.summary.is_empty() {
            root_node.summary.clone()
        } else {
            // Fallback: concatenate depth-1 summaries
            let mut parts = Vec::new();
            for child_id in tree.children(root) {
                if let Some(child) = tree.get(child_id) {
                    if !child.summary.is_empty() {
                        parts.push(format!("{}: {}", child.title, child.summary));
                    }
                }
            }
            parts.join("\n")
        };

        // Collect section summaries
        let mut section_summaries = Vec::new();
        for child_id in tree.children(root) {
            if let Some(child) = tree.get(child_id) {
                section_summaries.push(SectionSummary {
                    node_id: child_id,
                    title: child.title.clone(),
                    summary: child.summary.clone(),
                    depth: child.depth,
                });
            }
        }

        Some(SummaryShortcut {
            root_node: root,
            section_summaries,
            document_summary,
        })
    }
}

impl Default for ReasoningIndexStage {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl IndexStage for ReasoningIndexStage {
    fn name(&self) -> &'static str {
        "reasoning_index"
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
            writes_reasoning_index: true,
            ..Default::default()
        }
    }

    async fn execute(&mut self, ctx: &mut IndexContext) -> Result<StageResult> {
        let start = Instant::now();

        // Check if enabled via pipeline options
        if !ctx.options.reasoning_index.enabled {
            info!("Reasoning index stage disabled, skipping");
            return Ok(StageResult::success("reasoning_index"));
        }

        // Use stage config, overridden by pipeline options
        let config = &ctx.options.reasoning_index;

        let tree = match ctx.tree.as_ref() {
            Some(t) => t,
            None => {
                return Ok(StageResult::failure(
                    "reasoning_index",
                    "Tree not built",
                ));
            }
        };

        info!("Building reasoning index...");

        // 1. Build topic-to-path mapping
        let (topic_paths, keyword_count) = Self::build_topic_paths(tree, config);
        let topic_count: usize = topic_paths.values().map(|v| v.len()).sum();
        info!(
            "Built topic paths: {} keywords, {} topic entries",
            keyword_count, topic_count
        );

        // 2. Build section map
        let section_map = Self::build_section_map(tree);
        info!("Built section map with {} entries", section_map.len());

        // 3. Build summary shortcut
        let summary_shortcut = if config.build_summary_shortcut {
            let shortcut = Self::build_summary_shortcut(tree);
            if shortcut.is_some() {
                info!("Built summary shortcut");
            }
            shortcut
        } else {
            None
        };

        // 4. Assemble the reasoning index
        let mut builder = ReasoningIndexBuilder::new();
        for (keyword, entries) in topic_paths {
            for entry in entries {
                builder.add_topic_entry(&keyword, entry);
            }
        }
        for (title, node_id) in section_map {
            builder.add_section(&title, node_id);
        }
        if let Some(shortcut) = summary_shortcut {
            builder = builder.summary_shortcut(shortcut);
        }
        builder.sort_and_trim(config.max_topic_entries);

        let reasoning_index = builder.build();

        let duration = start.elapsed().as_millis() as u64;
        ctx.metrics
            .record_reasoning_index(duration, topic_count, keyword_count);

        info!(
            "Reasoning index built in {}ms ({} keywords, {} topic entries, {} sections)",
            duration,
            keyword_count,
            topic_count,
            reasoning_index.section_count(),
        );

        ctx.reasoning_index = Some(reasoning_index);

        let mut stage_result = StageResult::success("reasoning_index");
        stage_result.duration_ms = duration;
        stage_result.metadata.insert(
            "keywords_indexed".to_string(),
            serde_json::json!(keyword_count),
        );
        stage_result.metadata.insert(
            "topics_indexed".to_string(),
            serde_json::json!(topic_count),
        );

        Ok(stage_result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_node_keywords() {
        let keywords = ReasoningIndexStage::extract_node_keywords("Introduction to Machine Learning", 2);
        assert!(keywords.contains(&"introduction".to_string()));
        assert!(keywords.contains(&"machine".to_string()));
        assert!(keywords.contains(&"learning".to_string()));
    }

    #[test]
    fn test_extract_node_keywords_min_length() {
        let keywords = ReasoningIndexStage::extract_node_keywords("A B CD", 2);
        assert!(!keywords.contains(&"a".to_string()));
        assert!(!keywords.contains(&"b".to_string()));
        assert!(keywords.contains(&"cd".to_string()));
    }

    #[test]
    fn test_stage_config_default() {
        let stage = ReasoningIndexStage::new();
        assert!(stage.config.enabled);
        assert_eq!(stage.name(), "reasoning_index");
        assert!(stage.is_optional());
        assert_eq!(stage.depends_on(), vec!["enrich"]);
    }
}
