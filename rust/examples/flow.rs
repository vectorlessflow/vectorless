// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Complete Markdown processing flow example.
//!
//! This example demonstrates the full pipeline:
//! 1. Create a Vectorless client
//! 2. Index a Markdown document
//! 3. Show document structure in JSON format
//! 4. Query the document
//!
//! # Usage
//!
//! ```bash
//! cargo run --example flow
//! ```

use vectorless::EngineBuilder;
use vectorless::client::{IndexContext, IndexOptions, QueryContext};

/// Sample markdown content for demonstration.
const SAMPLE_MARKDOWN: &str = r#"
# Vectorless Architecture Guide

## Overview

Vectorless is a reasoning-native document intelligence engine that transforms documents into hierarchical semantic trees. Unlike traditional RAG systems that rely on vector embeddings and similarity search, Vectorless uses LLM-powered tree navigation to retrieve relevant content through deep contextual understanding.

The core idea is simple: structured documents already have inherent semantic relationships encoded in their headings, sections, and paragraphs. By preserving this structure as a navigable tree, an LLM can efficiently locate relevant information by following the document's own logical organization.

## Architecture

The system consists of three main components: an indexing pipeline, a storage layer, and a retrieval engine. The indexing pipeline parses documents into tree structures and generates summaries. The storage layer persists indexed documents to disk. The retrieval engine navigates the tree at query time using search algorithms guided by LLM decisions.

### Indexing Pipeline

The indexing pipeline processes documents through multiple stages: parsing, tree building, enhancement (LLM summary generation), and reasoning index construction. Each stage is independently configurable and can be enabled or disabled based on requirements. The pipeline supports incremental re-indexing with content fingerprinting to avoid redundant work when documents haven't changed.

### Retrieval Engine

The retrieval engine supports multiple search strategies including greedy depth-first search, beam search, and MCTS. A Pilot component provides LLM-guided navigation at key decision points during tree traversal. The engine is budget-aware, tracking token usage and making cost-conscious decisions about when to invoke the LLM versus using cheaper heuristic scoring.

## Performance

Under typical workloads, indexing a 50-page document takes approximately 10-30 seconds depending on LLM response latency and the complexity of the document structure. Query latency ranges from 200ms for simple keyword-matched queries to 3-5 seconds for complex multi-hop reasoning queries that require multiple LLM calls during tree navigation.

The system is designed for accuracy over speed. By leveraging document structure and LLM reasoning, it achieves higher retrieval quality than vector-based approaches on structured documents like technical reports, legal contracts, and research papers.
"#;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing for debug output (set RUST_LOG=debug to see more)
    tracing_subscriber::fmt::init();

    println!("=== Vectorless Flow Example ===\n");

    // Step 1: Create a Vectorless client
    println!("Step 1: Creating Vectorless client...");

    let client = EngineBuilder::new()
        .with_workspace("./workspace")
        .with_key("sk-...")
        .with_model("gpt-4o")
        .build()
        .await
        .map_err(|e: vectorless::BuildError| vectorless::Error::Config(e.to_string()))?;

    println!("  - Client created successfully");
    println!();

    // Step 2: Index the sample Markdown document
    println!("Step 2: Indexing Markdown document...");

    let temp_dir = tempfile::tempdir()?;
    let md_path = temp_dir.path().join("sample.md");
    tokio::fs::write(&md_path, SAMPLE_MARKDOWN).await?;

    let index_result = client
        .index(IndexContext::from_path(&md_path).with_options(IndexOptions::new().with_summaries()))
        .await?;
    let doc_id = index_result.doc_id().unwrap().to_string();

    println!("  - Document indexed successfully");
    println!("  - Document ID: {}", doc_id);
    println!();

    // Step 3: List indexed documents
    println!("Step 3: Indexed documents:");
    for doc in client.list().await? {
        println!("  - {} ({})", doc.name, doc.id);
    }
    println!();

    // Step 4: Query the document
    println!("Step 4: Querying the document...");

    let queries = vec!["What is this project about?"];

    for query in queries {
        println!("  Query: \"{}\"", query);

        match client
            .query(QueryContext::new(query).with_doc_id(&doc_id))
            .await
        {
            Ok(result) => {
                if let Some(item) = result.single() {
                    if item.content.is_empty() {
                        println!("    - No relevant content found");
                    } else {
                        println!("    - Found relevant content:");
                        let preview = if item.content.len() > 200 {
                            format!("{}...", &item.content[..200])
                        } else {
                            item.content.clone()
                        };
                        for line in preview.lines().take(5) {
                            println!("      {}", line);
                        }
                    }
                } else {
                    println!("    - No results");
                }
            }
            Err(e) => {
                println!("    - Error: {}", e);
            }
        }
        println!();
    }

    // Step 5: Cleanup
    println!("Step 5: Cleanup...");

    client.remove(&doc_id).await?;
    println!("  - Document removed");

    println!("\n=== Example Complete ===");
    Ok(())
}
