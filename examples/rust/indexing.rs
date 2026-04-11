// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Index pipeline example for Vectorless.
//!
//! Demonstrates the full indexing flow: create engine → index document → inspect metrics.
//!
//! # Usage
//!
//! ```bash
//! cargo run --example indexing
//! ```

use vectorless::{EngineBuilder, IndexContext, IndexMode};

#[tokio::main]
async fn main() -> vectorless::Result<()> {
    println!("=== Index Pipeline Example ===\n");

    // 1. Create engine
    let engine = EngineBuilder::new()
        .with_workspace("./workspace_index_example")
        .build()
        .await
        .map_err(|e: vectorless::BuildError| vectorless::Error::Config(e.to_string()))?;

    println!("Engine created\n");

    // 2. Index a single document with default options
    println!("--- Single document (default mode) ---");
    let result = engine.index(IndexContext::from_path("./README.md")).await?;

    for item in &result.items {
        println!("  doc_id:  {}", item.doc_id);
        println!("  name:    {}", item.name);
        println!("  format:  {:?}", item.format);

        if let Some(ref metrics) = item.metrics {
            println!("  metrics:");
            println!("    total time:  {}ms", metrics.total_time_ms());
            println!("    parse:       {}ms", metrics.parse_time_ms);
            println!("    build:       {}ms", metrics.build_time_ms);
            println!("    enhance:     {}ms", metrics.enhance_time_ms);
            println!("    enrich:      {}ms", metrics.enrich_time_ms);
            println!("    optimize:    {}ms", metrics.optimize_time_ms);
            println!("    reasoning:   {}ms", metrics.reasoning_index_time_ms);
            println!("    nodes:       {}", metrics.nodes_processed);
            println!("    summaries:   {}", metrics.summaries_generated);
            println!("    llm calls:   {}", metrics.llm_calls);
            println!("    tokens:      {}", metrics.total_tokens_generated);
            println!("    topics:      {}", metrics.topics_indexed);
            println!("    keywords:    {}", metrics.keywords_indexed);
        }

        // doc_id preserved across the loop for readability
        let _doc_id = item.doc_id.clone();

        // 3. Re-index with incremental mode — should detect no change
        println!("\n--- Re-index (incremental, unchanged) ---");
        let result2 = engine
            .index(IndexContext::from_path("./README.md").with_mode(IndexMode::Incremental))
            .await?;

        for item in &result2.items {
            println!(
                "  {} (metrics present: {})",
                item.doc_id,
                item.metrics.is_some()
            );
        }

        // 4. Index multiple documents at once
        println!("\n--- Batch indexing ---");
        let batch = engine
            .index(IndexContext::from_paths(&["./README.md", "./CLAUDE.md"]))
            .await?;

        println!(
            "  indexed: {}, failed: {}",
            batch.items.len(),
            batch.failed.len()
        );
        for item in &batch.items {
            let time = item
                .metrics
                .as_ref()
                .map(|m| m.total_time_ms())
                .unwrap_or(0);
            let nodes = item
                .metrics
                .as_ref()
                .map(|m| m.nodes_processed)
                .unwrap_or(0);
            println!("  {} — {}ms, {} nodes", item.name, time, nodes);
        }

        // 5. Cleanup
        println!("\n--- Cleanup ---");
        let docs = engine.list().await?;
        for doc in &docs {
            engine.remove(&doc.id).await?;
        }
        println!("  removed {} document(s)", docs.len());
    }

    println!("\n=== Done ===");
    Ok(())
}
