// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Document Graph data types.
//!
//! Core data structures for the workspace-scoped, weighted document relationship graph.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// A workspace-scoped document relationship graph.
///
/// Nodes represent documents, edges represent relationships (shared keywords,
/// references). The graph is immutable after construction and can be shared
/// across threads via `Arc`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentGraph {
    /// All document nodes, indexed by doc_id.
    nodes: HashMap<String, DocumentGraphNode>,

    /// Adjacency list: doc_id → outgoing edges.
    edges: HashMap<String, Vec<GraphEdge>>,

    /// Inverted index: keyword → documents containing this keyword.
    keyword_index: HashMap<String, Vec<KeywordDocEntry>>,

    /// Graph-level metadata.
    metadata: GraphMetadata,
}

/// Expose fields for graph builder (same module).
impl DocumentGraph {
    /// Take all edges out, leaving an empty map in their place.
    pub(crate) fn take_edges(&mut self) -> HashMap<String, Vec<GraphEdge>> {
        std::mem::take(&mut self.edges)
    }

    /// Set edges directly (used by builder after trimming).
    pub(crate) fn set_edges(&mut self, edges: HashMap<String, Vec<GraphEdge>>) {
        self.metadata.edge_count = edges.values().map(|v| v.len()).sum();
        self.edges = edges;
    }

    /// Get a clone of the keyword index (used by builder for edge computation).
    pub(crate) fn keyword_index_clone(&self) -> HashMap<String, Vec<KeywordDocEntry>> {
        self.keyword_index.clone()
    }
}

impl DocumentGraph {
    /// Create a new empty document graph.
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
            edges: HashMap::new(),
            keyword_index: HashMap::new(),
            metadata: GraphMetadata {
                document_count: 0,
                edge_count: 0,
            },
        }
    }

    /// Add a document node to the graph.
    pub fn add_node(&mut self, node: DocumentGraphNode) {
        // Populate keyword index from the node's top keywords
        for kw in &node.top_keywords {
            self.keyword_index
                .entry(kw.keyword.clone())
                .or_default()
                .push(KeywordDocEntry {
                    doc_id: node.doc_id.clone(),
                    weight: kw.weight,
                });
        }
        let doc_id = node.doc_id.clone();
        self.nodes.insert(doc_id, node);
        self.metadata.document_count = self.nodes.len();
    }

    /// Add a directed edge from `source` to `target`.
    pub fn add_edge(&mut self, source: &str, edge: GraphEdge) {
        self.edges.entry(source.to_string()).or_default().push(edge);
        self.metadata.edge_count = self.edges.values().map(|v| v.len()).sum();
    }

    /// Get a document node by ID.
    pub fn get_node(&self, doc_id: &str) -> Option<&DocumentGraphNode> {
        self.nodes.get(doc_id)
    }

    /// Get all edges outgoing from a document.
    pub fn get_neighbors(&self, doc_id: &str) -> &[GraphEdge] {
        self.edges.get(doc_id).map_or(&[], Vec::as_slice)
    }

    /// Find documents containing a keyword.
    pub fn find_by_keyword(&self, keyword: &str) -> &[KeywordDocEntry] {
        self.keyword_index.get(keyword).map_or(&[], Vec::as_slice)
    }

    /// Get the number of documents in the graph.
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// Get the number of edges in the graph.
    pub fn edge_count(&self) -> usize {
        self.edges.values().map(|v| v.len()).sum()
    }

    /// Get all document IDs in the graph.
    pub fn doc_ids(&self) -> impl Iterator<Item = &str> {
        self.nodes.keys().map(|s| s.as_str())
    }

    /// Get graph metadata.
    pub fn metadata(&self) -> &GraphMetadata {
        &self.metadata
    }

    /// Check if the graph is empty.
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }
}

impl Default for DocumentGraph {
    fn default() -> Self {
        Self::new()
    }
}

/// A document node in the graph.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentGraphNode {
    /// Document ID (matches `PersistedDocument.meta.id`).
    pub doc_id: String,
    /// Document title/name.
    pub title: String,
    /// Document format (md, pdf).
    pub format: String,
    /// Top-N representative keywords extracted from the document's
    /// ReasoningIndex topic_paths, sorted by aggregate weight.
    pub top_keywords: Vec<WeightedKeyword>,
    /// Number of nodes in the document tree.
    pub node_count: usize,
}

/// A keyword with its aggregate weight across the document.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeightedKeyword {
    /// The keyword string (lowercased).
    pub keyword: String,
    /// Aggregate weight across all TopicEntry instances (0.0 - 1.0).
    pub weight: f32,
}

/// An edge connecting two documents.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphEdge {
    /// Target document ID.
    pub target_doc_id: String,
    /// Edge weight (0.0 - 1.0). Higher = stronger relationship.
    pub weight: f32,
    /// Evidence for why these documents are connected.
    pub evidence: EdgeEvidence,
}

/// Evidence for why two documents are connected.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EdgeEvidence {
    /// Keywords shared between the two documents.
    pub shared_keywords: Vec<SharedKeyword>,
    /// Number of shared keywords.
    pub shared_keyword_count: usize,
    /// Jaccard similarity of keyword sets.
    pub keyword_jaccard: f32,
}

/// A keyword shared between two documents.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SharedKeyword {
    /// The shared keyword.
    pub keyword: String,
    /// Weight in source document.
    pub source_weight: f32,
    /// Weight in target document.
    pub target_weight: f32,
}

/// Entry in the keyword inverted index.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeywordDocEntry {
    /// Document ID containing this keyword.
    pub doc_id: String,
    /// Weight of this keyword in the document.
    pub weight: f32,
}

/// Graph-level metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphMetadata {
    /// Number of documents in the graph.
    pub document_count: usize,
    /// Number of edges in the graph.
    pub edge_count: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_graph() {
        let graph = DocumentGraph::new();
        assert!(graph.is_empty());
        assert_eq!(graph.node_count(), 0);
        assert_eq!(graph.edge_count(), 0);
    }

    #[test]
    fn test_add_node() {
        let mut graph = DocumentGraph::new();
        graph.add_node(DocumentGraphNode {
            doc_id: "doc1".to_string(),
            title: "Test Doc".to_string(),
            format: "md".to_string(),
            top_keywords: vec![
                WeightedKeyword {
                    keyword: "rust".to_string(),
                    weight: 0.9,
                },
                WeightedKeyword {
                    keyword: "async".to_string(),
                    weight: 0.7,
                },
            ],
            node_count: 10,
        });

        assert_eq!(graph.node_count(), 1);
        assert!(graph.get_node("doc1").is_some());
        assert_eq!(graph.find_by_keyword("rust").len(), 1);
        assert_eq!(graph.find_by_keyword("async").len(), 1);
        assert_eq!(graph.find_by_keyword("missing").len(), 0);
    }

    #[test]
    fn test_add_edge() {
        let mut graph = DocumentGraph::new();
        graph.add_node(DocumentGraphNode {
            doc_id: "doc1".to_string(),
            title: "A".to_string(),
            format: "md".to_string(),
            top_keywords: vec![],
            node_count: 5,
        });
        graph.add_node(DocumentGraphNode {
            doc_id: "doc2".to_string(),
            title: "B".to_string(),
            format: "md".to_string(),
            top_keywords: vec![],
            node_count: 8,
        });

        graph.add_edge(
            "doc1",
            GraphEdge {
                target_doc_id: "doc2".to_string(),
                weight: 0.5,
                evidence: EdgeEvidence {
                    shared_keywords: vec![SharedKeyword {
                        keyword: "rust".to_string(),
                        source_weight: 0.9,
                        target_weight: 0.8,
                    }],
                    shared_keyword_count: 1,
                    keyword_jaccard: 0.3,
                },
            },
        );

        assert_eq!(graph.edge_count(), 1);
        assert_eq!(graph.get_neighbors("doc1").len(), 1);
        assert_eq!(graph.get_neighbors("doc1")[0].target_doc_id, "doc2");
        assert_eq!(graph.get_neighbors("doc2").len(), 0);
    }

    #[test]
    fn test_serialization_roundtrip() {
        let mut graph = DocumentGraph::new();
        graph.add_node(DocumentGraphNode {
            doc_id: "doc1".to_string(),
            title: "Test".to_string(),
            format: "md".to_string(),
            top_keywords: vec![WeightedKeyword {
                keyword: "test".to_string(),
                weight: 1.0,
            }],
            node_count: 3,
        });

        let json = serde_json::to_string(&graph).unwrap();
        let deserialized: DocumentGraph = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.node_count(), 1);
        assert_eq!(deserialized.get_node("doc1").unwrap().title, "Test");
    }
}
