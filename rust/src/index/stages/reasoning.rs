// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Reasoning Index Stage - Build pre-computed reasoning index.
//!
//! This stage runs after EnrichStage (which generates descriptions and
//! calculates metadata) and before OptimizeStage. It builds a
//! [`ReasoningIndex`] from the document tree's TOC, summaries, and keywords.

use std::collections::HashMap;
use std::time::Instant;
use tracing::{debug, info, warn};

use crate::document::{
    NodeId, ReasoningIndexBuilder, ReasoningIndexConfig, SectionSummary, SummaryShortcut,
    TopicEntry,
};
use crate::error::Result;
use crate::llm::LlmClient;
use crate::scoring::extract_keywords;

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
    ) -> (HashMap<String, Vec<TopicEntry>>, usize) {
        let mut keyword_nodes: HashMap<String, Vec<(NodeId, f32, usize)>> = HashMap::new();

        // Walk all nodes and extract keywords from title + summary
        for node_id in tree.traverse() {
            if let Some(node) = tree.get(node_id) {
                let title_keywords =
                    Self::extract_node_keywords(&node.title, config.min_keyword_length);
                let summary_keywords =
                    Self::extract_node_keywords(&node.summary, config.min_keyword_length);
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
        let mut topic_paths: HashMap<String, Vec<TopicEntry>> = HashMap::new();

        for (keyword, entries) in sorted_keywords {
            // Merge duplicate node entries by summing weights
            let mut merged: HashMap<NodeId, (f32, usize)> = HashMap::new();
            for (node_id, weight, depth) in entries {
                let entry = merged.entry(node_id).or_insert((0.0, depth));
                entry.0 += weight;
            }

            // Normalize weights to 0.0-1.0 range
            let max_weight = merged.values().map(|(w, _)| *w).fold(0.0_f32, f32::max);
            let scale = if max_weight > 0.0 {
                1.0 / max_weight
            } else {
                1.0
            };

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
    fn build_section_map(tree: &crate::document::DocumentTree) -> HashMap<String, NodeId> {
        let mut section_map = HashMap::new();
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

    /// Expand keywords with LLM-generated synonyms (single batch request).
    ///
    /// Sends all keywords to the LLM in one request and maps each to its
    /// synonyms. Synonym entries inherit the same node mappings but with
    /// a reduced weight (0.6x) to reflect the indirect match.
    async fn expand_synonyms(
        topic_paths: &mut HashMap<String, Vec<TopicEntry>>,
        llm_client: &LlmClient,
        max_keywords: usize,
    ) -> usize {
        use std::collections::HashSet;

        let existing_keys: HashSet<String> = topic_paths.keys().cloned().collect();
        // Pick top keywords by entry count for synonym expansion
        let mut ranked: Vec<(String, usize)> = topic_paths
            .iter()
            .map(|(k, v): (&String, &Vec<TopicEntry>)| (k.clone(), v.len()))
            .collect();
        ranked.sort_by(|a, b| b.1.cmp(&a.1));
        ranked.truncate(max_keywords);

        let keyword_count = ranked.len();
        if keyword_count == 0 {
            return 0;
        }

        tracing::info!(
            "[reasoning_index] Expanding synonyms for {} keywords (single request)",
            keyword_count,
        );

        // Snapshot the source entries for each keyword.
        let source_entries: HashMap<String, Vec<TopicEntry>> = ranked
            .iter()
            .map(|(kw, _): &(String, usize)| {
                (kw.clone(), topic_paths.get(kw).cloned().unwrap_or_default())
            })
            .collect();

        let keywords: Vec<String> = ranked.into_iter().map(|(kw, _)| kw).collect();

        let system = "You are a thesaurus assistant. For each keyword, provide up to 5 synonyms \
            or related search terms. Return ONLY a valid JSON object mapping each keyword to an \
            array of synonym strings. No explanation, no markdown.";
        let user_prompt = format!(
            "Keywords: {}\n\nReturn a JSON object: {{\"keyword\": [\"syn1\", \"syn2\"], ...}}",
            keywords.join(", ")
        );

        let synonym_map: HashMap<String, Vec<String>> =
            match llm_client.complete_json::<HashMap<String, Vec<String>>>(system, &user_prompt).await {
                Ok(map) => map
                    .into_iter()
                    .map(|(k, v): (String, Vec<String>)| (k.to_lowercase(), v))
                    .collect(),
                Err(e) => {
                    tracing::warn!(
                        "[reasoning_index] Batch synonym expansion failed: {}",
                        e
                    );
                    return 0;
                }
            };

        // Write results back
        let mut synonym_count = 0;
        for keyword in &keywords {
            if let Some(synonyms) = synonym_map.get(keyword) {
                if let Some(entries) = source_entries.get(keyword) {
                    for syn in synonyms {
                        let syn_clean = syn.trim().to_lowercase();
                        if syn_clean.is_empty() || syn_clean.len() < 2 || existing_keys.contains(&syn_clean) {
                            continue;
                        }
                        let synonym_entries: Vec<TopicEntry> = entries
                            .iter()
                            .map(|e| TopicEntry {
                                node_id: e.node_id,
                                weight: e.weight * 0.6,
                                depth: e.depth,
                            })
                            .collect();
                        topic_paths.insert(syn_clean, synonym_entries);
                        synonym_count += 1;
                    }
                }
            }
        }

        synonym_count
    }

    /// Build summary shortcut from root and depth-1 nodes.
    fn build_summary_shortcut(tree: &crate::document::DocumentTree) -> Option<SummaryShortcut> {
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
            info!("[reasoning_index] Disabled, skipping");
            return Ok(StageResult::success("reasoning_index"));
        }

        // Use stage config, overridden by pipeline options
        let config = &ctx.options.reasoning_index;

        let tree = match ctx.tree.as_ref() {
            Some(t) => t,
            None => {
                warn!("[reasoning_index] No tree, cannot build index");
                return Ok(StageResult::failure("reasoning_index", "Tree not built"));
            }
        };

        info!(
            "[reasoning_index] Starting: synonyms={}, summary_shortcut={}, max_keywords={}",
            config.enable_synonym_expansion,
            config.build_summary_shortcut,
            config.max_keyword_entries,
        );

        // 1. Build topic-to-path mapping
        let (mut topic_paths, keyword_count) = Self::build_topic_paths(tree, config);
        let topic_count: usize = topic_paths
            .values()
            .map(|v: &Vec<TopicEntry>| v.len())
            .sum();
        debug!(
            "[reasoning_index] Topic paths: {} keywords, {} entries",
            keyword_count, topic_count
        );

        // 1b. Optional: expand keywords with LLM-generated synonyms
        let synonym_count = if config.enable_synonym_expansion {
            if let Some(ref llm_client) = ctx.llm_client {
                let max_kw = (keyword_count / 4).max(20).min(100);
                let count =
                    Self::expand_synonyms(&mut topic_paths, llm_client, max_kw).await;
                if count > 0 {
                    info!("[reasoning_index] Expanded {} synonym keywords", count);
                }
                count
            } else {
                debug!("[reasoning_index] Synonym expansion enabled but no LLM client");
                0
            }
        } else {
            0
        };

        // 2. Build section map
        let section_map = Self::build_section_map(tree);
        debug!(
            "[reasoning_index] Section map: {} entries",
            section_map.len()
        );

        // 3. Build summary shortcut
        let summary_shortcut = if config.build_summary_shortcut {
            let shortcut = Self::build_summary_shortcut(tree);
            if shortcut.is_some() {
                debug!("[reasoning_index] Built summary shortcut");
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
            "[reasoning_index] Complete: {} keywords, {} topics, {} sections, {} synonyms in {}ms",
            keyword_count,
            topic_count,
            reasoning_index.section_count(),
            synonym_count,
            duration,
        );

        ctx.reasoning_index = Some(reasoning_index);

        let mut stage_result = StageResult::success("reasoning_index");
        stage_result.duration_ms = duration;
        stage_result.metadata.insert(
            "keywords_indexed".to_string(),
            serde_json::json!(keyword_count),
        );
        stage_result
            .metadata
            .insert("topics_indexed".to_string(), serde_json::json!(topic_count));
        stage_result.metadata.insert(
            "synonyms_expanded".to_string(),
            serde_json::json!(synonym_count),
        );

        Ok(stage_result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_node_keywords() {
        let keywords =
            ReasoningIndexStage::extract_node_keywords("Introduction to Machine Learning", 2);
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

    #[test]
    fn test_build_topic_paths_basic() {
        use crate::document::ReasoningIndexConfig;

        let mut tree = crate::document::DocumentTree::new("Root", "");
        let root = tree.root();
        let c1 = tree.add_child(root, "Machine Learning Introduction", "");
        let c2 = tree.add_child(root, "Deep Learning Methods", "");

        // Set summaries for keyword extraction
        if let Some(n) = tree.get_mut(c1) {
            n.summary = "An overview of machine learning algorithms".to_string();
        }
        if let Some(n) = tree.get_mut(c2) {
            n.summary = "Advanced deep learning techniques".to_string();
        }

        let config = ReasoningIndexConfig::default();
        let (topic_paths, keyword_count) = ReasoningIndexStage::build_topic_paths(&tree, &config);

        assert!(
            keyword_count > 0,
            "Should extract keywords from title + summary"
        );
        assert!(!topic_paths.is_empty(), "Should build topic paths");

        // "learning" appears in both titles → should be a keyword
        assert!(
            topic_paths.contains_key("learning"),
            "Expected 'learning' in topic paths, got: {:?}",
            topic_paths.keys().collect::<Vec<_>>()
        );
    }

    #[test]
    fn test_build_topic_paths_weight_normalization() {
        use crate::document::ReasoningIndexConfig;

        let mut tree = crate::document::DocumentTree::new("Root", "");
        let root = tree.root();
        let _c1 = tree.add_child(root, "rust ownership", "rust borrowing rules");

        let config = ReasoningIndexConfig::default();
        let (topic_paths, _) = ReasoningIndexStage::build_topic_paths(&tree, &config);

        // All weights should be in 0.0-1.0 range
        for entries in topic_paths.values() {
            for entry in entries {
                assert!(
                    entry.weight >= 0.0 && entry.weight <= 1.0,
                    "Weight {} out of [0, 1] range",
                    entry.weight
                );
            }
        }
    }

    #[test]
    fn test_build_topic_paths_respects_max_keyword_entries() {
        use crate::document::ReasoningIndexConfig;

        let mut tree = crate::document::DocumentTree::new("Root", "");
        let root = tree.root();

        // Create many children with unique keywords
        for i in 0..50 {
            let c = tree.add_child(root, &format!("Section {} Alpha Beta Gamma Delta", i), "");
            if let Some(n) = tree.get_mut(c) {
                n.summary = format!("keywords unique{} special{} terms{}", i, i, i);
            }
        }

        let mut config = ReasoningIndexConfig::default();
        config.max_keyword_entries = 5;
        let (topic_paths, keyword_count) = ReasoningIndexStage::build_topic_paths(&tree, &config);

        assert!(
            keyword_count <= 5,
            "Should respect max_keyword_entries, got {}",
            keyword_count
        );
        assert_eq!(topic_paths.len(), keyword_count);
    }

    #[test]
    fn test_build_section_map() {
        let mut tree = crate::document::DocumentTree::new("Root", "");
        let root = tree.root();
        let c1 = tree.add_child(root, "Introduction", "content");
        let c2 = tree.add_child(root, "Methods", "content");

        // Set structure indices
        if let Some(n) = tree.get_mut(c1) {
            n.structure = "1".to_string();
        }
        if let Some(n) = tree.get_mut(c2) {
            n.structure = "2".to_string();
        }

        let section_map = ReasoningIndexStage::build_section_map(&tree);

        // Should index by title (lowercase) and structure index
        assert!(section_map.contains_key("introduction"));
        assert!(section_map.contains_key("methods"));
        assert!(section_map.contains_key("1"));
        assert!(section_map.contains_key("2"));
        assert_eq!(section_map.len(), 4);
    }

    #[test]
    fn test_build_summary_shortcut() {
        let mut tree = crate::document::DocumentTree::new("Root", "");
        let root = tree.root();
        let c1 = tree.add_child(root, "S1", "summary 1");
        let c2 = tree.add_child(root, "S2", "summary 2");

        // Set root summary (not content — build_summary_shortcut reads summary field)
        if let Some(n) = tree.get_mut(root) {
            n.summary = "root summary text".to_string();
        }
        if let Some(n) = tree.get_mut(c1) {
            n.summary = "first section summary".to_string();
        }
        if let Some(n) = tree.get_mut(c2) {
            n.summary = "second section summary".to_string();
        }

        let shortcut = ReasoningIndexStage::build_summary_shortcut(&tree);
        assert!(shortcut.is_some());

        let sc = shortcut.unwrap();
        assert_eq!(sc.root_node, root);
        assert_eq!(sc.document_summary, "root summary text");
        assert_eq!(sc.section_summaries.len(), 2);
    }

    #[test]
    fn test_build_summary_shortcut_fallback_to_children() {
        // Root has no summary → fallback to concatenating children
        let mut tree = crate::document::DocumentTree::new("Root", "");
        let root = tree.root();
        let c1 = tree.add_child(root, "S1", "");
        let c2 = tree.add_child(root, "S2", "");

        if let Some(n) = tree.get_mut(c1) {
            n.summary = "child summary 1".to_string();
        }
        if let Some(n) = tree.get_mut(c2) {
            n.summary = "child summary 2".to_string();
        }

        let shortcut = ReasoningIndexStage::build_summary_shortcut(&tree);
        assert!(shortcut.is_some());

        let sc = shortcut.unwrap();
        assert!(
            sc.document_summary.contains("child summary 1"),
            "Fallback should include child summaries"
        );
        assert!(sc.document_summary.contains("S1"));
    }
}
