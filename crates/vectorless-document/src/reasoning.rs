// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Pre-computed reasoning index for fast retrieval path resolution.
//!
//! Built at compile time from TOC and summaries, the reasoning index provides
//! topic-to-path mappings, summary shortcuts, and hot node tracking that
//! accelerate query-time retrieval by bypassing expensive tree traversal.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use super::node::NodeId;

/// A pre-computed reasoning index that maps topics and query patterns
/// to optimal tree paths, built at compile time for query-time acceleration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReasoningIndex {
    /// Keyword → list of (NodeId, weight) entries.
    /// Built from titles and summaries at compile time.
    /// Key = lowercased keyword token.
    topic_paths: HashMap<String, Vec<TopicEntry>>,

    /// Pre-computed shortcut for "document summary" queries.
    /// Maps summary-type query patterns directly to the root node
    /// and its top-level children summaries.
    summary_shortcut: Option<SummaryShortcut>,

    /// Nodes marked as hot (frequently retrieved).
    /// NodeId → cumulative hit count and rolling average score.
    /// Uses `node_id_map` because serde_json cannot deserialize
    /// `HashMap<NodeId, _>` (integer keys are incompatible with JSON).
    #[serde(with = "super::serde_helpers")]
    hot_nodes: HashMap<NodeId, HotNodeEntry>,

    /// Depth-1 section title → NodeId mapping for fast ToC lookup.
    section_map: HashMap<String, NodeId>,

    /// Configuration used to build this index (for cache invalidation).
    #[serde(default)]
    config_hash: u64,
}

impl ReasoningIndex {
    /// Create a new empty reasoning index.
    pub fn new() -> Self {
        Self {
            topic_paths: HashMap::new(),
            summary_shortcut: None,
            hot_nodes: HashMap::new(),
            section_map: HashMap::new(),
            config_hash: 0,
        }
    }

    /// Create a builder for constructing the reasoning index.
    pub fn builder() -> ReasoningIndexBuilder {
        ReasoningIndexBuilder::new()
    }

    /// Look up topic entries for a keyword.
    pub fn topic_entries(&self, keyword: &str) -> Option<&[TopicEntry]> {
        self.topic_paths.get(keyword).map(Vec::as_slice)
    }

    /// Get the summary shortcut, if available.
    pub fn summary_shortcut(&self) -> Option<&SummaryShortcut> {
        self.summary_shortcut.as_ref()
    }

    /// Check if a node is marked as hot.
    pub fn is_hot(&self, node_id: NodeId) -> bool {
        self.hot_nodes
            .get(&node_id)
            .map(|e| e.is_hot)
            .unwrap_or(false)
    }

    /// Get the hot node entry for a node.
    pub fn hot_entry(&self, node_id: NodeId) -> Option<&HotNodeEntry> {
        self.hot_nodes.get(&node_id)
    }

    /// Look up a section by its title.
    pub fn find_section(&self, title: &str) -> Option<NodeId> {
        self.section_map.get(&title.to_lowercase()).copied()
    }

    /// Iterate over all keyword → topic entries (for graph building).
    pub fn all_topic_entries(&self) -> impl Iterator<Item = (&String, &[TopicEntry])> {
        self.topic_paths.iter().map(|(k, v)| (k, v.as_slice()))
    }

    /// Get the number of topic keywords indexed.
    pub fn topic_count(&self) -> usize {
        self.topic_paths.len()
    }

    /// Get the number of sections in the section map.
    pub fn section_count(&self) -> usize {
        self.section_map.len()
    }

    /// Get the number of hot nodes.
    pub fn hot_node_count(&self) -> usize {
        self.hot_nodes.iter().filter(|(_, e)| e.is_hot).count()
    }

    /// Update hot node tracking from retrieval results.
    pub fn update_hot_nodes(&mut self, hits: &[(NodeId, f32)], hot_threshold: u32) {
        for &(node_id, score) in hits {
            let entry = self.hot_nodes.entry(node_id).or_insert(HotNodeEntry {
                hit_count: 0,
                avg_score: 0.0,
                is_hot: false,
            });
            entry.hit_count += 1;
            entry.avg_score += (score - entry.avg_score) / entry.hit_count as f32;
            if entry.hit_count >= hot_threshold {
                entry.is_hot = true;
            }
        }
    }
}

impl Default for ReasoningIndex {
    fn default() -> Self {
        Self::new()
    }
}

/// Builder for constructing a `ReasoningIndex`.
pub struct ReasoningIndexBuilder {
    topic_paths: HashMap<String, Vec<TopicEntry>>,
    summary_shortcut: Option<SummaryShortcut>,
    hot_nodes: HashMap<NodeId, HotNodeEntry>,
    section_map: HashMap<String, NodeId>,
    config_hash: u64,
}

impl ReasoningIndexBuilder {
    /// Create a new builder.
    pub fn new() -> Self {
        Self {
            topic_paths: HashMap::new(),
            summary_shortcut: None,
            hot_nodes: HashMap::new(),
            section_map: HashMap::new(),
            config_hash: 0,
        }
    }

    /// Add a topic entry for a keyword.
    pub fn add_topic_entry(&mut self, keyword: impl Into<String>, entry: TopicEntry) {
        self.topic_paths
            .entry(keyword.into())
            .or_default()
            .push(entry);
    }

    /// Set the summary shortcut.
    pub fn summary_shortcut(mut self, shortcut: SummaryShortcut) -> Self {
        self.summary_shortcut = Some(shortcut);
        self
    }

    /// Add a section mapping.
    pub fn add_section(&mut self, title: impl Into<String>, node_id: NodeId) {
        self.section_map
            .insert(title.into().to_lowercase(), node_id);
    }

    /// Set the config hash for cache invalidation.
    pub fn config_hash(mut self, hash: u64) -> Self {
        self.config_hash = hash;
        self
    }

    /// Sort topic entries by weight (descending) and trim per-keyword lists.
    pub fn sort_and_trim(&mut self, max_entries: usize) {
        for entries in self.topic_paths.values_mut() {
            entries.sort_by(|a, b| {
                b.weight
                    .partial_cmp(&a.weight)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            entries.truncate(max_entries);
        }
    }

    /// Build the reasoning index.
    pub fn build(self) -> ReasoningIndex {
        ReasoningIndex {
            topic_paths: self.topic_paths,
            summary_shortcut: self.summary_shortcut,
            hot_nodes: self.hot_nodes,
            section_map: self.section_map,
            config_hash: self.config_hash,
        }
    }
}

impl Default for ReasoningIndexBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// A topic entry mapping a keyword to a node with a weight.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopicEntry {
    /// The target node.
    pub node_id: NodeId,
    /// Weight indicating how relevant this keyword is to this node (0.0 - 1.0).
    pub weight: f32,
    /// Depth of the node in the tree (for tie-breaking).
    pub depth: usize,
}

/// Pre-computed shortcut for summary-style queries.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SummaryShortcut {
    /// The root node ID (direct answer for "what is this about" queries).
    pub root_node: NodeId,
    /// Pre-collected summaries of top-level sections.
    pub section_summaries: Vec<SectionSummary>,
    /// Combined summary text for direct return.
    pub document_summary: String,
}

/// A pre-collected section summary for quick access.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SectionSummary {
    /// Section node ID.
    pub node_id: NodeId,
    /// Section title.
    pub title: String,
    /// Section summary (pre-computed by EnhanceStage).
    pub summary: String,
    /// Depth of the section.
    pub depth: usize,
}

/// Entry tracking how often a node is retrieved.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HotNodeEntry {
    /// Number of times this node appeared in retrieval results.
    pub hit_count: u32,
    /// Rolling average score when retrieved.
    pub avg_score: f32,
    /// Whether this node is currently marked as "hot"
    /// (hit_count exceeds configured threshold).
    pub is_hot: bool,
}

/// Configuration for building and using the reasoning index.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReasoningIndexConfig {
    /// Whether reasoning index building is enabled.
    pub enabled: bool,
    /// Minimum hit count for a node to be considered "hot".
    pub hot_node_threshold: u32,
    /// Maximum number of topic entries per keyword.
    pub max_topic_entries: usize,
    /// Maximum number of keyword-to-node mappings to keep.
    pub max_keyword_entries: usize,
    /// Minimum keyword length to index.
    pub min_keyword_length: usize,
    /// Whether to build the summary shortcut.
    pub build_summary_shortcut: bool,
    /// Whether to expand keywords with LLM-generated synonyms.
    /// When enabled, the compile stage calls the LLM to generate
    /// synonym terms for each keyword, improving recall for queries
    /// that use different wording than the document.
    pub enable_synonym_expansion: bool,
}

impl Default for ReasoningIndexConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            hot_node_threshold: 3,
            max_topic_entries: 20,
            max_keyword_entries: 5000,
            min_keyword_length: 2,
            build_summary_shortcut: true,
            enable_synonym_expansion: true,
        }
    }
}

impl ReasoningIndexConfig {
    /// Create a new config with defaults.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a disabled config.
    pub fn disabled() -> Self {
        Self {
            enabled: false,
            ..Self::default()
        }
    }

    /// Set the hot node threshold.
    pub fn with_hot_threshold(mut self, threshold: u32) -> Self {
        self.hot_node_threshold = threshold;
        self
    }

    /// Set whether to build the summary shortcut.
    pub fn with_summary_shortcut(mut self, build: bool) -> Self {
        self.build_summary_shortcut = build;
        self
    }

    /// Enable or disable synonym expansion.
    pub fn with_synonym_expansion(mut self, enable: bool) -> Self {
        self.enable_synonym_expansion = enable;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reasoning_index_default() {
        let index = ReasoningIndex::default();
        assert_eq!(index.topic_count(), 0);
        assert_eq!(index.section_count(), 0);
        assert_eq!(index.hot_node_count(), 0);
        assert!(index.summary_shortcut().is_none());
    }

    #[test]
    fn test_builder_basic() {
        // Create a simple tree to get valid NodeIds
        let mut tree = crate::tree::DocumentTree::new("Root", "root content");
        let child1 = tree.add_child(tree.root(), "Introduction", "intro content");
        let child2 = tree.add_child(tree.root(), "Methods", "methods content");

        let mut builder = ReasoningIndexBuilder::new();
        builder.add_section("Introduction", child1);
        builder.add_section("Methods", child2);

        let index = builder.build();
        assert_eq!(index.section_count(), 2);
        assert!(index.find_section("introduction").is_some());
        assert!(index.find_section("INTRODUCTION").is_some());
        assert!(index.find_section("methods").is_some());
    }

    #[test]
    fn test_config_default() {
        let config = ReasoningIndexConfig::default();
        assert!(config.enabled);
        assert_eq!(config.hot_node_threshold, 3);
        assert!(config.build_summary_shortcut);
    }

    #[test]
    fn test_config_disabled() {
        let config = ReasoningIndexConfig::disabled();
        assert!(!config.enabled);
    }

    #[test]
    fn test_serialization_roundtrip_empty() {
        let mut tree = crate::tree::DocumentTree::new("Root", "content");
        let child = tree.add_child(tree.root(), "Section 1", "s1 content");

        let mut builder = ReasoningIndexBuilder::new();
        builder.add_section("Section 1", child);
        builder.add_topic_entry(
            "section",
            TopicEntry {
                node_id: child,
                weight: 0.8,
                depth: 1,
            },
        );
        let index = builder.build();

        let json = serde_json::to_string(&index).unwrap();
        let restored: ReasoningIndex = serde_json::from_str(&json).unwrap();

        assert_eq!(restored.topic_count(), 1);
        assert_eq!(restored.section_count(), 1);
        assert_eq!(restored.hot_node_count(), 0);
    }

    #[test]
    fn test_serialization_roundtrip_with_hot_nodes() {
        let mut tree = crate::tree::DocumentTree::new("Root", "");
        let root = tree.root();
        let c1 = tree.add_child(root, "S1", "content 1");
        let c2 = tree.add_child(root, "S2", "content 2");

        let mut index = ReasoningIndex::new();
        index.update_hot_nodes(&[(c1, 0.9), (c2, 0.7), (c1, 0.8)], 2);

        // c1 should be hot (2 hits >= threshold 2)
        assert!(index.is_hot(c1));
        // c2 should not be hot (1 hit < threshold 2)
        assert!(!index.is_hot(c2));

        let json = serde_json::to_string(&index).unwrap();

        // hot_nodes should serialize as array of pairs, not as object
        assert!(!json.contains("\"hot_nodes\":{}"));
        assert!(json.contains("\"hot_nodes\":["));

        let restored: ReasoningIndex = serde_json::from_str(&json).unwrap();
        assert!(restored.is_hot(c1));
        assert!(!restored.is_hot(c2));

        let entry = restored.hot_entry(c1).unwrap();
        assert_eq!(entry.hit_count, 2);
        assert!(entry.avg_score > 0.0);
    }

    #[test]
    fn test_backward_compat_hot_nodes_empty_object() {
        // Simulate old JSON where hot_nodes was serialized as {} by derive.
        let mut tree = crate::tree::DocumentTree::new("Root", "");
        let child = tree.add_child(tree.root(), "S1", "c");

        let mut builder = ReasoningIndexBuilder::new();
        builder.add_section("s1", child);
        let index = builder.build();

        // Serialize normally (produces "hot_nodes":[]), then replace with
        // the old format to test backward compat
        let json = serde_json::to_string(&index).unwrap();
        let old_json = json.replace("\"hot_nodes\":[]", "\"hot_nodes\":{}");

        let restored: ReasoningIndex = serde_json::from_str(&old_json).unwrap();
        assert_eq!(restored.hot_node_count(), 0);
    }
}
