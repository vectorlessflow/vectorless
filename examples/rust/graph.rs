// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Document graph example for Vectorless.
//!
//! Demonstrates how to retrieve the cross-document relationship graph
//! after indexing. The graph is automatically rebuilt after each index call,
//! connecting documents that share keywords via Jaccard similarity.
//!
//! # Usage
//!
//! ```bash
//! cargo run --example graph
//! ```

use vectorless::{EngineBuilder, IndexContext};

#[tokio::main]
async fn main() -> vectorless::Result<()> {
    println!("=== Document Graph Example ===\n");

    // 1. Create engine
    let engine = EngineBuilder::new()
        .with_workspace("./workspace_graph_example")
        .build()
        .await
        .map_err(|e: vectorless::BuildError| vectorless::Error::Config(e.to_string()))?;

    // 2. Index documents — graph is rebuilt automatically
    let result = engine
        .index(IndexContext::from_paths(&["./README.md", "./CLAUDE.md"]))
        .await?;

    println!("Indexed {} document(s)", result.items.len());
    for item in &result.items {
        println!("  - {} ({})", item.name, item.doc_id);
    }
    println!();

    // 3. Get the document graph
    match engine.get_graph().await? {
        Some(graph) => {
            println!(
                "Document graph: {} nodes, {} edges",
                graph.node_count(),
                graph.edge_count()
            );

            // Show document nodes
            for doc_id in graph.doc_ids() {
                if let Some(node) = graph.get_node(doc_id) {
                    println!(
                        "  Node: {} — {} keyword(s), top: {:?}",
                        node.title,
                        node.top_keywords.len(),
                        node.top_keywords
                            .iter()
                            .take(3)
                            .map(|kw| &kw.keyword)
                            .collect::<Vec<_>>()
                    );

                    // Show edges (connected documents)
                    let neighbors = graph.get_neighbors(doc_id);
                    if !neighbors.is_empty() {
                        for edge in neighbors {
                            println!(
                                "    → {} (weight={:.2}, jaccard={:.2}, shared={})",
                                edge.target_doc_id,
                                edge.weight,
                                edge.evidence.keyword_jaccard,
                                edge.evidence.shared_keyword_count,
                            );
                        }
                    } else {
                        println!("    (no connections)");
                    }
                }
            }
        }
        None => println!("No graph available (no documents with reasoning index)"),
    }

    // 4. Cleanup
    let docs = engine.list().await?;
    for doc in &docs {
        engine.remove(&doc.id).await?;
    }

    println!("\n=== Done ===");
    Ok(())
}
