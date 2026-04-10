// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Document Graph example.
//!
//! Demonstrates how to:
//! 1. Build a document graph from multiple documents
//! 2. Explore cross-document relationships (shared keywords, edges)
//! 3. Use graph-aware retrieval with different merge strategies
//!
//! # What is a Document Graph?
//!
//! A workspace-scoped weighted graph connecting documents by shared concepts.
//! Nodes = documents, Edges = relationships (shared keywords with weights).
//!
//! # Key outputs:
//! - Document nodes with top keywords
//! - Bidirectional edges with Jaccard similarity and shared keyword evidence
//! - Keyword inverted index for cross-document lookup
//! - Graph-boosted retrieval ranking
//!
//! # Usage
//!
//! ```bash
//! cargo run --example document_graph
//! ```

use std::collections::HashMap;

use vectorless::document::{
    DocumentGraph, DocumentGraphConfig, DocumentGraphNode, WeightedKeyword,
};
use vectorless::index::graph_builder::DocumentGraphBuilder;

#[tokio::main]
async fn main() {
    println!("=== Document Graph Example ===\n");

    // -------------------------------------------------------
    // Part 1: Build the graph manually (low-level API)
    // -------------------------------------------------------
    println!("--- Part 1: Build Graph Manually ---\n");
    demo_manual_graph();

    // -------------------------------------------------------
    // Part 2: Build the graph with DocumentGraphBuilder
    // -------------------------------------------------------
    println!("\n--- Part 2: Build Graph with Builder ---\n");
    let graph = demo_builder();

    // -------------------------------------------------------
    // Part 3: Explore the graph
    // -------------------------------------------------------
    println!("\n--- Part 3: Explore the Graph ---\n");
    demo_explore(&graph);

    // -------------------------------------------------------
    // Part 4: Keyword-based document lookup
    // -------------------------------------------------------
    println!("\n--- Part 4: Keyword Lookup ---\n");
    demo_keyword_lookup(&graph);

    // -------------------------------------------------------
    // Part 5: Show graph-boosted retrieval concept
    // -------------------------------------------------------
    println!("\n--- Part 5: Graph-Boosted Retrieval ---\n");
    demo_graph_boosted_retrieval(&graph);

    println!("\n=== Done ===");
}

/// Manually build a small graph to show the data model.
fn demo_manual_graph() {
    let mut graph = DocumentGraph::new();

    // Add document nodes
    graph.add_node(DocumentGraphNode {
        doc_id: "rust-book".to_string(),
        title: "The Rust Programming Language".to_string(),
        format: "md".to_string(),
        top_keywords: vec![
            WeightedKeyword { keyword: "ownership".to_string(), weight: 0.95 },
            WeightedKeyword { keyword: "borrowing".to_string(), weight: 0.90 },
            WeightedKeyword { keyword: "lifetimes".to_string(), weight: 0.80 },
            WeightedKeyword { keyword: "traits".to_string(), weight: 0.70 },
        ],
        node_count: 42,
    });

    graph.add_node(DocumentGraphNode {
        doc_id: "rust-async".to_string(),
        title: "Async Programming in Rust".to_string(),
        format: "md".to_string(),
        top_keywords: vec![
            WeightedKeyword { keyword: "async".to_string(), weight: 0.95 },
            WeightedKeyword { keyword: "tokio".to_string(), weight: 0.85 },
            WeightedKeyword { keyword: "lifetimes".to_string(), weight: 0.60 },
            WeightedKeyword { keyword: "traits".to_string(), weight: 0.50 },
        ],
        node_count: 28,
    });

    println!("Nodes: {}", graph.node_count());
    for doc_id in graph.doc_ids() {
        let node = graph.get_node(doc_id).unwrap();
        println!("  {} ({}): {} keywords, {} nodes",
            node.doc_id, node.title, node.top_keywords.len(), node.node_count);
    }
}

/// Build a graph from multiple documents using DocumentGraphBuilder.
fn demo_builder() -> DocumentGraph {
    let config = DocumentGraphConfig {
        enabled: true,
        min_keyword_jaccard: 0.05,
        min_shared_keywords: 2,
        max_keywords_per_doc: 50,
        max_edges_per_node: 20,
        retrieval_boost_factor: 0.15,
    };

    let mut builder = DocumentGraphBuilder::new(config);

    // Document 1: Rust Language Guide
    builder.add_document(
        "rust-guide",
        "Rust Language Guide",
        "md",
        35,
        keywords(&[
            ("ownership", 0.95), ("borrowing", 0.90), ("lifetimes", 0.85),
            ("traits", 0.80), ("generics", 0.75), ("error-handling", 0.70),
            ("pattern-matching", 0.65), ("closures", 0.60),
        ]),
    );

    // Document 2: Async Rust (overlaps on lifetimes, traits, closures)
    builder.add_document(
        "async-guide",
        "Async Rust Guide",
        "md",
        28,
        keywords(&[
            ("async", 0.95), ("tokio", 0.90), ("futures", 0.85),
            ("lifetimes", 0.60), ("traits", 0.55), ("closures", 0.50),
            ("pinning", 0.80), ("waker", 0.75),
        ]),
    );

    // Document 3: Rust Testing (overlaps on traits, closures, error-handling)
    builder.add_document(
        "testing-guide",
        "Rust Testing Guide",
        "md",
        22,
        keywords(&[
            ("testing", 0.95), ("assertions", 0.90), ("mocking", 0.85),
            ("traits", 0.60), ("closures", 0.55), ("error-handling", 0.50),
            ("benchmarks", 0.80), ("coverage", 0.75),
        ]),
    );

    // Document 4: Unrelated document (cooking — no overlap)
    builder.add_document(
        "cooking",
        "Italian Cooking",
        "md",
        15,
        keywords(&[
            ("pasta", 0.95), ("sauce", 0.90), ("olive-oil", 0.85),
            ("garlic", 0.80), ("basil", 0.75), ("tomato", 0.70),
        ]),
    );

    let graph = builder.build();

    println!("Graph built:");
    println!("  Documents: {}", graph.node_count());
    println!("  Edges:     {}", graph.edge_count());

    graph
}

/// Explore nodes, edges, and relationship evidence.
fn demo_explore(graph: &DocumentGraph) {
    for doc_id in graph.doc_ids() {
        let node = graph.get_node(doc_id).unwrap();
        let neighbors = graph.get_neighbors(doc_id);

        println!("[{}] {} ({} nodes)", node.doc_id, node.title, node.node_count);

        // Show top keywords
        let top_3: Vec<String> = node.top_keywords.iter()
            .take(3)
            .map(|kw| format!("{} ({:.2})", kw.keyword, kw.weight))
            .collect();
        println!("  Keywords: {}", top_3.join(", "));

        // Show edges to other documents
        if neighbors.is_empty() {
            println!("  Edges: (none — isolated document)");
        } else {
            println!("  Edges:");
            for edge in neighbors {
                println!(
                    "    -> {} [weight={:.3}, jaccard={:.3}, shared={}]",
                    edge.target_doc_id,
                    edge.weight,
                    edge.evidence.keyword_jaccard,
                    edge.evidence.shared_keyword_count,
                );
                // Show shared keywords
                let shared: Vec<String> = edge.evidence.shared_keywords.iter()
                    .map(|sk| format!("{} ({:.2}/{:.2})", sk.keyword, sk.source_weight, sk.target_weight))
                    .collect();
                println!("       Shared: {}", shared.join(", "));
            }
        }
        println!();
    }
}

/// Look up documents by keyword using the inverted index.
fn demo_keyword_lookup(graph: &DocumentGraph) {
    let queries = ["traits", "closures", "async", "pasta", "nonexistent"];

    for kw in &queries {
        let entries = graph.find_by_keyword(kw);
        if entries.is_empty() {
            println!("  '{}': not found in any document", kw);
        } else {
            let docs: Vec<String> = entries.iter()
                .map(|e| format!("{} ({:.2})", e.doc_id, e.weight))
                .collect();
            println!("  '{}': found in {}", kw, docs.join(", "));
        }
    }
}

/// Show how graph-boosted retrieval works conceptually.
fn demo_graph_boosted_retrieval(graph: &DocumentGraph) {
    println!("Scenario: User queries 'traits and closures'");
    println!();

    // Step 1: Simulate per-document scores
    let results = vec![
        ("rust-guide".to_string(), 0.85),
        ("async-guide".to_string(), 0.60),
        ("testing-guide".to_string(), 0.55),
        ("cooking".to_string(), 0.10),
    ];

    println!("Before graph boosting:");
    for (doc, score) in &results {
        println!("  {}: {:.3}", doc, score);
    }

    // Step 2: Apply graph boost — high-score docs boost their neighbors
    let boost_factor = 0.15;
    let mut boosted = results.clone();
    for (doc, base_score) in &results {
        if *base_score > 0.5 {
            for edge in graph.get_neighbors(doc) {
                for entry in boosted.iter_mut() {
                    if entry.0 == edge.target_doc_id {
                        let boost = boost_factor * edge.weight * base_score;
                        entry.1 += boost;
                    }
                }
            }
        }
    }
    boosted.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

    println!();
    println!("After graph boosting (boost_factor={}):", boost_factor);
    for (doc, score) in &boosted {
        let delta = score - results.iter().find(|(d, _)| d == doc).unwrap().1;
        println!("  {}: {:.3} (+{:.3})", doc, score, delta);
    }

    println!();
    println!("Effect: Related documents (rust-guide, async-guide, testing-guide)");
    println!("  boost each other via shared keywords, while 'cooking' stays low.");
}

// Helper to build keyword maps
fn keywords(pairs: &[(&str, f32)]) -> HashMap<String, f32> {
    pairs.iter().map(|&(k, w)| (k.to_string(), w)).collect()
}
