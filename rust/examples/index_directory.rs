// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Directory indexing example — recursively index all documents in a directory.
//!
//! ```bash
//! # Using environment variables for LLM config:
//! LLM_API_KEY=sk-xxx LLM_MODEL=google/gemini-3-flash-preview \
//!   LLM_ENDPOINT=http://localhost:4000/api/v1 \
//!   cargo run --example index_directory -- /path/to/docs
//!
//! # With recursive flag (default):
//! cargo run --example index_directory -- /path/to/docs --recursive
//!
//! # Non-recursive (top-level only):
//! cargo run --example index_directory -- /path/to/docs --no-recursive
//! ```

use vectorless::{EngineBuilder, IndexContext};

#[tokio::main]
async fn main() -> vectorless::Result<()> {
    tracing_subscriber::fmt::init();

    // Parse CLI arguments
    let args: Vec<String> = std::env::args().collect();
    let dir = args
        .get(1)
        .map(|s| s.as_str())
        .unwrap_or("./samples");
    let recursive = !args.iter().any(|a| a == "--no-recursive");

    // Build engine
    let api_key = std::env::var("LLM_API_KEY").unwrap_or_else(|_| "sk-or-v1-...".to_string());
    let model =
        std::env::var("LLM_MODEL").unwrap_or_else(|_| "google/gemini-3-flash-preview".to_string());
    let endpoint = std::env::var("LLM_ENDPOINT")
        .unwrap_or_else(|_| "http://localhost:4000/api/v1".to_string());

    let engine = EngineBuilder::new()
        .with_workspace("./workspace_directory_example")
        .with_key(&api_key)
        .with_model(&model)
        .with_endpoint(&endpoint)
        .build()
        .await
        .map_err(|e| vectorless::Error::Config(e.to_string()))?;

    // Index directory
    let ctx = if recursive {
        println!("Recursively indexing: {}", dir);
        IndexContext::from_dir_recursive(dir)
    } else {
        println!("Indexing top-level files in: {}", dir);
        IndexContext::from_dir(dir)
    };

    if ctx.is_empty() {
        println!("No supported files found in: {}", dir);
        return Ok(());
    }

    println!("Found {} file(s) to index", ctx.len());

    let result = engine.index(ctx).await?;

    println!("\nIndexed {} document(s):", result.items.len());
    for item in &result.items {
        println!("  {} ({})", item.name, item.doc_id);
        if let Some(metrics) = &item.metrics {
            println!(
                "    nodes: {}, time: {}ms",
                metrics.nodes_processed,
                metrics.total_time_ms()
            );
        }
    }

    if result.has_failures() {
        println!("\nFailed:");
        for f in &result.failed {
            println!("  {} — {}", f.source, f.error);
        }
    }

    // Query across all indexed documents
    let query = "What is this about?";
    println!("\nQuerying: \"{query}\"");

    let answer = engine
        .query(vectorless::QueryContext::new(query))
        .await?;

    for item in &answer.items {
        println!("  [{} score={:.2}]", item.doc_id, item.score);
        let preview: String = item.content.chars().take(200).collect();
        println!("  {preview}");
        if item.content.len() > 200 {
            println!("  ...");
        }
    }

    // Metrics report
    let report = engine.metrics_report();
    println!("\nMetrics:");
    println!(
        "  LLM: {} calls, {} tokens, ${:.4}",
        report.llm.total_calls,
        report.llm.total_tokens,
        report.llm.estimated_cost_usd,
    );
    println!(
        "  Retrieval: {} queries, avg score {:.2}",
        report.retrieval.total_queries, report.retrieval.avg_path_score,
    );

    // Cleanup
    for doc in engine.list().await? {
        engine.remove(&doc.id).await?;
    }

    Ok(())
}
