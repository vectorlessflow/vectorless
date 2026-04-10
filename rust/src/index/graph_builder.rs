// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Document Graph Builder — constructs cross-document relationship graphs.
//!
//! This is a standalone builder (not an `IndexStage`) because it operates
//! on the workspace level across all documents, not on a single document.

use std::collections::HashMap;

use tracing::info;

use crate::document::{
    DocumentGraph, DocumentGraphConfig, DocumentGraphNode, EdgeEvidence, GraphEdge, SharedKeyword,
    WeightedKeyword,
};

/// Intermediate data collected per document during graph building.
#[derive(Debug, Clone)]
struct DocProfile {
    doc_id: String,
    title: String,
    format: String,
    node_count: usize,
    /// keyword → aggregate weight
    keywords: HashMap<String, f32>,
}

/// Builder for constructing a `DocumentGraph` from multiple documents.
pub struct DocumentGraphBuilder {
    config: DocumentGraphConfig,
    profiles: Vec<DocProfile>,
}

impl DocumentGraphBuilder {
    /// Create a new builder with the given configuration.
    pub fn new(config: DocumentGraphConfig) -> Self {
        Self {
            config,
            profiles: Vec::new(),
        }
    }

    /// Create a builder with default configuration.
    pub fn with_defaults() -> Self {
        Self::new(DocumentGraphConfig::default())
    }

    /// Add a document's keyword profile to the builder.
    ///
    /// `keywords` should map keyword → aggregate weight (from
    /// `ReasoningIndex::topic_paths` or extracted from content).
    pub fn add_document(
        &mut self,
        doc_id: impl Into<String>,
        title: impl Into<String>,
        format: impl Into<String>,
        node_count: usize,
        keywords: HashMap<String, f32>,
    ) {
        self.profiles.push(DocProfile {
            doc_id: doc_id.into(),
            title: title.into(),
            format: format.into(),
            node_count,
            keywords,
        });
    }

    /// Build the document graph from accumulated document profiles.
    pub fn build(self) -> DocumentGraph {
        let mut graph = DocumentGraph::new();

        if self.profiles.is_empty() {
            info!("Building document graph: 0 documents, empty graph");
            return graph;
        }

        // Step 1: Add document nodes with top-N keywords
        for profile in &self.profiles {
            let mut weighted: Vec<WeightedKeyword> = profile
                .keywords
                .iter()
                .map(|(kw, &w)| WeightedKeyword {
                    keyword: kw.clone(),
                    weight: w,
                })
                .collect();
            // Sort by weight descending
            weighted.sort_by(|a, b| {
                b.weight
                    .partial_cmp(&a.weight)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            weighted.truncate(self.config.max_keywords_per_doc);

            graph.add_node(DocumentGraphNode {
                doc_id: profile.doc_id.clone(),
                title: profile.title.clone(),
                format: profile.format.clone(),
                top_keywords: weighted,
                node_count: profile.node_count,
            });
        }

        info!(
            "Building document graph: {} document nodes added",
            graph.node_count()
        );

        // Step 2: Compute edges using the keyword inverted index
        // (already built inside graph.add_node via keyword_index)
        self.compute_edges(&mut graph);

        info!(
            "Document graph built: {} nodes, {} edges",
            graph.node_count(),
            graph.edge_count()
        );

        graph
    }

    /// Compute edges between documents based on shared keywords.
    fn compute_edges(&self, graph: &mut DocumentGraph) {
        // Collect candidate pairs: (doc_a, doc_b) → shared keywords
        let mut pair_shared: HashMap<(String, String), Vec<SharedKeyword>> = HashMap::new();

        // Iterate the keyword index: for each keyword, all docs sharing it are candidates
        let kw_index = graph.keyword_index_clone();

        for (keyword, entries) in &kw_index {
            if entries.len() < 2 {
                continue; // No pair possible
            }
            // For every pair of documents sharing this keyword
            for i in 0..entries.len() {
                for j in (i + 1)..entries.len() {
                    let a = &entries[i];
                    let b = &entries[j];
                    let pair = if a.doc_id < b.doc_id {
                        (a.doc_id.clone(), b.doc_id.clone())
                    } else {
                        (b.doc_id.clone(), a.doc_id.clone())
                    };
                    let shared = SharedKeyword {
                        keyword: keyword.clone(),
                        source_weight: a.weight,
                        target_weight: b.weight,
                    };
                    pair_shared.entry(pair).or_default().push(shared);
                }
            }
        }

        // Step 3: Create edges for pairs that meet thresholds
        for ((doc_a, doc_b), shared_kws) in pair_shared {
            let shared_count = shared_kws.len();
            if shared_count < self.config.min_shared_keywords {
                continue;
            }

            // Compute Jaccard: |intersection| / |union|
            let kw_a = graph
                .get_node(&doc_a)
                .map(|n| n.top_keywords.len())
                .unwrap_or(0);
            let kw_b = graph
                .get_node(&doc_b)
                .map(|n| n.top_keywords.len())
                .unwrap_or(0);
            let union_size = kw_a + kw_b - shared_count;
            let jaccard = if union_size > 0 {
                shared_count as f32 / union_size as f32
            } else {
                0.0
            };

            if jaccard < self.config.min_keyword_jaccard {
                continue;
            }

            // Edge weight: combine Jaccard with keyword count
            let max_kws = self.config.max_keywords_per_doc.max(1) as f32;
            let weight = (jaccard * 0.6 + (shared_count as f32 / max_kws).min(1.0) * 0.4).min(1.0);

            // Create bidirectional edges
            let evidence_a = EdgeEvidence {
                shared_keywords: shared_kws.clone(),
                shared_keyword_count: shared_count,
                keyword_jaccard: jaccard,
            };
            let evidence_b = EdgeEvidence {
                shared_keywords: shared_kws
                    .iter()
                    .map(|s| SharedKeyword {
                        keyword: s.keyword.clone(),
                        source_weight: s.target_weight,
                        target_weight: s.source_weight,
                    })
                    .collect(),
                shared_keyword_count: shared_count,
                keyword_jaccard: jaccard,
            };

            graph.add_edge(
                &doc_a,
                GraphEdge {
                    target_doc_id: doc_b.clone(),
                    weight,
                    evidence: evidence_a,
                },
            );
            graph.add_edge(
                &doc_b,
                GraphEdge {
                    target_doc_id: doc_a.clone(),
                    weight,
                    evidence: evidence_b,
                },
            );
        }

        // Step 4: Trim edges per node to max_edges_per_node
        self.trim_edges(graph);
    }

    /// Trim edges per node to the configured maximum.
    fn trim_edges(&self, graph: &mut DocumentGraph) {
        let max = self.config.max_edges_per_node;
        let all_edges = graph.take_edges();
        let mut trimmed: HashMap<String, Vec<GraphEdge>> = HashMap::new();

        for (source, mut edges) in all_edges {
            edges.sort_by(|a, b| {
                b.weight
                    .partial_cmp(&a.weight)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            edges.truncate(max);
            trimmed.insert(source, edges);
        }

        graph.set_edges(trimmed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_keywords(pairs: &[(&str, f32)]) -> HashMap<String, f32> {
        pairs
            .iter()
            .map(|&(k, w)| (k.to_string(), w))
            .collect()
    }

    #[test]
    fn test_empty_workspace() {
        let builder = DocumentGraphBuilder::with_defaults();
        let graph = builder.build();
        assert!(graph.is_empty());
    }

    #[test]
    fn test_single_document() {
        let mut builder = DocumentGraphBuilder::with_defaults();
        builder.add_document(
            "doc1",
            "Test",
            "md",
            5,
            make_keywords(&[("rust", 0.9), ("async", 0.7)]),
        );
        let graph = builder.build();
        assert_eq!(graph.node_count(), 1);
        assert_eq!(graph.edge_count(), 0);
    }

    #[test]
    fn test_two_docs_shared_keywords() {
        let mut builder = DocumentGraphBuilder::new(DocumentGraphConfig {
            min_keyword_jaccard: 0.05,
            min_shared_keywords: 2,
            ..DocumentGraphConfig::default()
        });
        builder.add_document(
            "doc1",
            "Rust Programming",
            "md",
            10,
            make_keywords(&[("rust", 0.9), ("async", 0.8), ("tokio", 0.6)]),
        );
        builder.add_document(
            "doc2",
            "Async Rust",
            "md",
            8,
            make_keywords(&[("rust", 0.7), ("async", 0.9), ("futures", 0.5)]),
        );

        let graph = builder.build();
        assert_eq!(graph.node_count(), 2);
        // Should have bidirectional edges
        assert!(graph.edge_count() >= 2);

        // Check doc1 → doc2 edge
        let neighbors = graph.get_neighbors("doc1");
        assert_eq!(neighbors.len(), 1);
        assert_eq!(neighbors[0].target_doc_id, "doc2");
        assert!(neighbors[0].weight > 0.0);
        assert!(neighbors[0].evidence.keyword_jaccard > 0.0);
        assert!(neighbors[0].evidence.shared_keyword_count >= 2);

        // Check doc2 → doc1 edge (bidirectional)
        let neighbors2 = graph.get_neighbors("doc2");
        assert_eq!(neighbors2.len(), 1);
        assert_eq!(neighbors2[0].target_doc_id, "doc1");
    }

    #[test]
    fn test_unrelated_docs_no_edge() {
        let mut builder = DocumentGraphBuilder::new(DocumentGraphConfig {
            min_keyword_jaccard: 0.1,
            min_shared_keywords: 2,
            ..DocumentGraphConfig::default()
        });
        builder.add_document(
            "doc1",
            "Rust Guide",
            "md",
            10,
            make_keywords(&[("rust", 0.9), ("ownership", 0.8)]),
        );
        builder.add_document(
            "doc2",
            "Cooking Recipes",
            "md",
            8,
            make_keywords(&[("pasta", 0.9), ("sauce", 0.8)]),
        );

        let graph = builder.build();
        assert_eq!(graph.node_count(), 2);
        assert_eq!(graph.edge_count(), 0);
    }

    #[test]
    fn test_jaccard_threshold() {
        let mut builder = DocumentGraphBuilder::new(DocumentGraphConfig {
            min_keyword_jaccard: 0.9, // Very high threshold
            min_shared_keywords: 1,
            ..DocumentGraphConfig::default()
        });
        // Two docs with minimal overlap
        builder.add_document(
            "doc1",
            "A",
            "md",
            5,
            make_keywords(&[
                ("a", 0.9),
                ("b", 0.8),
                ("c", 0.7),
                ("d", 0.6),
                ("e", 0.5),
            ]),
        );
        builder.add_document(
            "doc2",
            "B",
            "md",
            5,
            make_keywords(&[("a", 0.9), ("x", 0.8), ("y", 0.7), ("z", 0.6)]),
        );

        let graph = builder.build();
        // Only 1 shared keyword out of 5+4=9 unique, Jaccard = 1/8 ≈ 0.125
        // Way below 0.9 threshold → no edge
        assert_eq!(graph.edge_count(), 0);
    }

    #[test]
    fn test_max_edges_per_node() {
        let mut builder = DocumentGraphBuilder::new(DocumentGraphConfig {
            min_keyword_jaccard: 0.01,
            min_shared_keywords: 1,
            max_edges_per_node: 2,
            ..DocumentGraphConfig::default()
        });

        // 4 docs all sharing keywords with doc1
        for i in 0..4 {
            builder.add_document(
                format!("doc{}", i),
                format!("Doc {}", i),
                "md",
                5,
                make_keywords(&[("shared", 0.9), ("common", 0.8)]),
            );
        }

        let graph = builder.build();
        // doc1 should have at most 2 outgoing edges
        let neighbors = graph.get_neighbors("doc0");
        assert!(neighbors.len() <= 2);
    }
}
