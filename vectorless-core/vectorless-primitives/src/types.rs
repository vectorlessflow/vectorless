// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Return types for document navigation primitives.

/// Information about a node in the document tree.
#[derive(Debug, Clone)]
pub struct NodeInfo {
    /// Numeric identifier (usable as `"n{id}"` in Python).
    pub id: u64,
    /// Section title.
    pub title: String,
    /// Depth in the tree (0 = root).
    pub depth: usize,
    /// Number of direct children.
    pub child_count: usize,
    /// Number of leaf descendants.
    pub leaf_count: usize,
    /// Questions this subtree can answer.
    pub question_hints: Vec<String>,
    /// Topic tags for routing.
    pub topic_tags: Vec<String>,
}

/// A regex match within a node's content.
#[derive(Debug, Clone)]
pub struct MatchResult {
    /// Node containing the match.
    pub node_id: u64,
    /// Title of the matched node.
    pub title: String,
    /// The matching line of content.
    pub snippet: String,
    /// 1-based line number within the node's content.
    pub line_number: usize,
}

/// A node found by title or content search.
#[derive(Debug, Clone)]
pub struct FindResult {
    /// Numeric identifier.
    pub node_id: u64,
    /// Section title.
    pub title: String,
    /// Depth in the tree.
    pub depth: usize,
    /// Number of leaf descendants.
    pub leaf_count: usize,
}

/// Word/line/character count for a node's content.
#[derive(Debug, Clone)]
pub struct WordCount {
    /// Number of lines.
    pub lines: usize,
    /// Number of whitespace-separated words.
    pub words: usize,
    /// Number of characters.
    pub chars: usize,
}

/// Evidence collected from a node during navigation.
#[derive(Debug, Clone)]
pub struct CollectedEvidence {
    /// Node the evidence was collected from.
    pub node_id: u64,
    /// Title of the node.
    pub title: String,
    /// Full content of the node.
    pub content: String,
    /// Navigation path (e.g., "root / Chapter 1 / Section 1.2").
    pub source_path: String,
}

/// A topic entry from the reasoning index.
#[derive(Debug, Clone)]
pub struct TopicEntryInfo {
    /// Node associated with this entry.
    pub node_id: u64,
    /// Relevance weight.
    pub weight: f32,
    /// Depth in the tree.
    pub depth: usize,
}

/// A section summary from the reasoning index.
#[derive(Debug, Clone)]
pub struct SectionSummaryInfo {
    /// Node this summary belongs to.
    pub node_id: u64,
    /// Section title.
    pub title: String,
    /// LLM-generated summary.
    pub summary: String,
    /// Depth in the tree.
    pub depth: usize,
}

// ---------------------------------------------------------------------------
// P1: New types for extended agent tools
// ---------------------------------------------------------------------------

/// A single entry in the table of contents.
#[derive(Debug, Clone)]
pub struct TocEntry {
    /// Numeric identifier (usable as `"n{id}"` in Python).
    pub id: u64,
    /// Section title.
    pub title: String,
    /// Depth in the tree (1 = top-level section, 0 = root which is skipped).
    pub depth: usize,
    /// Number of direct children.
    pub child_count: usize,
}

/// Statistics about a single node.
#[derive(Debug, Clone)]
pub struct NodeStats {
    /// Numeric identifier.
    pub id: u64,
    /// Section title.
    pub title: String,
    /// Depth in the tree.
    pub depth: usize,
    /// Number of direct children.
    pub child_count: usize,
    /// Number of leaf descendants.
    pub leaf_count: usize,
    /// Character count of the node's content.
    pub char_count: usize,
    /// Word count of the node's content.
    pub word_count: usize,
    /// Whether this node has no children.
    pub is_leaf: bool,
}

/// A node found by semantic similarity.
#[derive(Debug, Clone)]
pub struct SimilarResult {
    /// Numeric identifier.
    pub id: u64,
    /// Section title.
    pub title: String,
    /// Combined relevance score.
    pub relevance: f32,
    /// Keywords shared with the reference node.
    pub shared_keywords: Vec<String>,
}

/// One top-level section in a [`DocCardInfo`].
#[derive(Debug, Clone)]
pub struct SectionCardInfo {
    /// Section title.
    pub title: String,
    /// One-sentence description of this section.
    pub description: String,
    /// Number of leaf nodes in this section's subtree.
    pub leaf_count: usize,
}

/// Document-level overview card.
#[derive(Debug, Clone)]
pub struct DocCardInfo {
    /// Document title.
    pub title: String,
    /// Document overview summary.
    pub overview: String,
    /// Questions this document can answer.
    pub question_hints: Vec<String>,
    /// Topic keywords.
    pub topic_tags: Vec<String>,
    /// Top-level section summaries.
    pub sections: Vec<SectionCardInfo>,
    /// Total leaf nodes in the document.
    pub total_leaves: usize,
}

/// A key concept extracted from the document.
#[derive(Debug, Clone)]
pub struct ConceptInfo {
    /// Concept name (e.g., "capacitor derating").
    pub name: String,
    /// One-sentence explanation.
    pub summary: String,
    /// Which sections this concept appears in.
    pub sections: Vec<String>,
}
