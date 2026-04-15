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
//! # Using environment variables for LLM config:
//! LLM_API_KEY=sk-xxx LLM_MODEL=gpt-4o \
//!   LLM_ENDPOINT=https://api.openai.com/v1 cargo run --example events
//!
//! # Or with defaults (edit the code to set your key/endpoint):
//! cargo run --example events
//! ```

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use vectorless::client::{EngineBuilder, IndexContext, QueryContext};
use vectorless::events::{EventEmitter, IndexEvent, QueryEvent};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing for debug output (set RUST_LOG=debug to see more)
    tracing_subscriber::fmt::init();

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

    // Build engine with LLM configuration from environment or defaults.
    // Adjust the defaults below to match your setup.
    let api_key = std::env::var("LLM_API_KEY").unwrap_or_else(|_| "sk-...".to_string());
    let model = std::env::var("LLM_MODEL").unwrap_or_else(|_| "gpt-4o".to_string());
    let endpoint =
        std::env::var("LLM_ENDPOINT").unwrap_or_else(|_| "https://api.openai.com/v1".to_string());

    // 2. Create engine with events
    println!("Step 2: Creating engine with event emitter...");
    let engine = EngineBuilder::new()
        .with_key(&api_key)
        .with_model(&model)
        .with_endpoint(&endpoint)
        .with_events(events)
        .build()
        .await?;
    println!("  ✓ Engine created\n");

    // 3. Index a document with events
    println!("Step 3: Indexing document (with events)...");
    let result = engine
        .index(IndexContext::from_path("../README.md"))
        .await?;
    let doc_id = result.doc_id().unwrap().to_string();
    println!("  ✓ Indexed: {doc_id}\n");

    // 4. Query with events
    println!("Step 4: Querying (with events)...");
    let result = engine
        .query(QueryContext::new("What is vectorless?").with_doc_ids(vec![doc_id.clone()]))
        .await?;
    if let Some(item) = result.single() {
        println!("  ✓ Found result ({} chars)", item.content.len());
        if !item.content.is_empty() {
            let preview: String = item.content.chars().take(200).collect();
            println!("  Preview: {}...", preview);
        }
    }

    // 5. Stats
    println!("\n--- Stats ---");
    println!(
        "  Documents indexed: {}",
        index_count.load(Ordering::SeqCst)
    );
    println!("  Queries executed: {}", query_count.load(Ordering::SeqCst));
    println!("  Nodes visited: {}", nodes_visited.load(Ordering::SeqCst));

    // Cleanup
    engine.remove(&doc_id).await?;
    println!("\n  Cleaned up");

    println!("\n=== Done ===");
    Ok(())
}
