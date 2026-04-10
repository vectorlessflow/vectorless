// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Event callbacks example.
//!
//! This example demonstrates the event system for:
//! - Monitoring indexing progress
//! - Tracking query execution
//! - Debugging retrieval behavior
//!
//! # Usage
//!
//! ```bash
//! cargo run --example events
//! ```

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use vectorless::client::{EngineBuilder, EventEmitter, IndexContext, IndexEvent, QueryContext, QueryEvent};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Event Callbacks Example ===\n");

    // 1. Create event emitter with handlers
    println!("Step 1: Setting up event handlers...\n");

    let index_count = Arc::new(AtomicUsize::new(0));
    let query_count = Arc::new(AtomicUsize::new(0));
    let nodes_visited = Arc::new(AtomicUsize::new(0));

    let index_count_clone = index_count.clone();
    let query_count_clone = query_count.clone();
    let nodes_visited_clone = nodes_visited.clone();

    let events = EventEmitter::new()
        // Index events
        .on_index(move |e| match e {
            IndexEvent::Started { path } => {
                println!("  [INDEX] Started: {}", path);
            }
            IndexEvent::FormatDetected { format } => {
                println!("  [INDEX] Format: {:?}", format);
            }
            IndexEvent::TreeBuilt { node_count } => {
                println!("  [INDEX] Tree built: {} nodes", node_count);
            }
            IndexEvent::Complete { doc_id } => {
                println!("  [INDEX] Complete: {}", &doc_id[..8]);
                index_count_clone.fetch_add(1, Ordering::SeqCst);
            }
            IndexEvent::Error { message } => {
                println!("  [INDEX] Error: {}", message);
            }
            _ => {}
        })
        // Query events
        .on_query(move |e| match e {
            QueryEvent::Started { query } => {
                println!("  [QUERY] Started: \"{}\"", query);
                query_count_clone.fetch_add(1, Ordering::SeqCst);
            }
            QueryEvent::NodeVisited { title, score, .. } => {
                println!("  [QUERY] Visited: \"{}\" (score: {:.2})", title, score);
                nodes_visited_clone.fetch_add(1, Ordering::SeqCst);
            }
            QueryEvent::CandidateFound { node_id, score } => {
                println!(
                    "  [QUERY] Candidate: {} (score: {:.2})",
                    &node_id[..8],
                    score
                );
            }
            QueryEvent::Complete {
                total_results,
                confidence,
            } => {
                println!(
                    "  [QUERY] Complete: {} results, confidence: {:.2}",
                    total_results, confidence
                );
            }
            QueryEvent::Error { message } => {
                println!("  [QUERY] Error: {}", message);
            }
            _ => {}
        });

    println!("  ✓ Event handlers configured\n");

    // 2. Create engine with events
    println!("Step 2: Creating engine with event emitter...");
    let engine = EngineBuilder::new()
        .with_workspace("./workspace_events_example")
        .with_events(events)
        .build()
        .await
        .map_err(|e: vectorless::BuildError| vectorless::Error::Config(e.to_string()))?;
    println!("  ✓ Engine created\n");

    // 3. Index a document (events will fire)
    println!("Step 3: Indexing document (watch events)...\n");

    let temp_dir = tempfile::tempdir()?;
    let doc_content = r#"# Example Document

## Introduction

This is an example document for demonstrating event callbacks.

## Features

- Event monitoring for indexing
- Event monitoring for queries
- Progress tracking

## Architecture

The event system uses handlers that can be attached to the engine builder.
"#;

    let doc_path = temp_dir.path().join("example.md");
    tokio::fs::write(&doc_path, doc_content).await?;

    let index_result = engine.index(IndexContext::from_path(&doc_path)).await?;
    let doc_id = index_result.doc_id().unwrap().to_string();
    println!();

    // 4. Query the document (events will fire)
    println!("Step 4: Querying document (watch events)...\n");

    let result = engine
        .query(QueryContext::new("What features are available?").with_doc_id(&doc_id))
        .await?;
    println!();

    // 5. Show results
    println!("Step 5: Query result:");
    println!("  - Score: {:.2}", result.score);
    println!("  - Nodes: {}", result.node_ids.len());
    if !result.content.is_empty() {
        let preview: String = result.content.chars().take(100).collect();
        println!("  - Content: {}...", preview);
    }
    println!();

    // 6. Show statistics
    println!("Step 6: Event statistics:");
    println!(
        "  - Index events fired: {}",
        index_count.load(Ordering::SeqCst)
    );
    println!(
        "  - Query events fired: {}",
        query_count.load(Ordering::SeqCst)
    );
    println!(
        "  - Nodes visited: {}",
        nodes_visited.load(Ordering::SeqCst)
    );
    println!();

    // 7. Cleanup
    println!("Step 7: Cleanup...");
    engine.remove(&doc_id).await?;
    println!("  ✓ Document removed\n");

    println!("=== Example Complete ===");
    Ok(())
}
