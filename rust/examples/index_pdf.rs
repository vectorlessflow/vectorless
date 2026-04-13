// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! PDF indexing example — index a PDF document via the vectorless engine.
//!
//! ```bash
//! # Using environment variables for LLM config:
//! LLM_API_KEY=sk-xxx LLM_MODEL=google/gemini-3-flash-preview \
//!   cargo run --example index_pdf -- ../samples/Docker_Cheat_Sheet.pdf
//!
//! # Or with defaults (edit the code to set your key/endpoint):
//! cargo run --example index_pdf -- ../samples/Docker_Cheat_Sheet.pdf
//! ```

use std::path::Path;

use vectorless::{EngineBuilder, IndexContext};

#[tokio::main]
async fn main() -> vectorless::Result<()> {
    let args: Vec<String> = std::env::args().collect();

    let pdf_path = args.get(1).map(|s| s.as_str()).unwrap_or_else(|| {
        eprintln!("Usage: cargo run --example index_pdf -- <path-to-pdf>");
        std::process::exit(1);
    });

    if !Path::new(pdf_path).exists() {
        eprintln!("Error: file not found: {}", pdf_path);
        std::process::exit(1);
    }

    println!("=== Indexing PDF: {} ===\n", pdf_path);

    // Build engine with LLM configuration from environment or defaults.
    // Adjust the defaults below to match your setup.
    let api_key = std::env::var("LLM_API_KEY")
        .unwrap_or_else(|_| "sk-or-v1-...".to_string());
    let model = std::env::var("LLM_MODEL")
        .unwrap_or_else(|_| "google/gemini-3-flash-preview".to_string());
    let endpoint = std::env::var("LLM_ENDPOINT")
        .unwrap_or_else(|_| "http://localhost:4000/api/v1".to_string());

    let engine = EngineBuilder::new()
        .with_workspace("./workspace_pdf_example")
        .with_key(&api_key)
        .with_model(&model)
        .with_endpoint(&endpoint)
        .build()
        .await
        .map_err(|e| vectorless::Error::Config(e.to_string()))?;

    // Index the PDF — format is auto-detected from the .pdf extension.
    // The engine will:
    //   1. Extract text from every page
    //   2. Detect and parse the Table of Contents
    //   3. Build a hierarchical document tree
    //   4. Generate summaries for each node (LLM)
    //   5. Build a reasoning index for retrieval
    let result = engine
        .index(IndexContext::from_path(pdf_path))
        .await?;

    println!(
        "Indexed: {}, Failed: {}",
        result.items.len(),
        result.failed.len()
    );

    for item in &result.items {
        println!("\n--- {} ---", item.name);
        println!("doc_id:  {}", item.doc_id);
        println!("format:  {:?}", item.format);

        if let Some(metrics) = &item.metrics {
            println!("\nMetrics:");
            println!("  total time:    {}ms", metrics.total_time_ms());
            println!("  parse:         {}ms", metrics.parse_time_ms);
            println!("  build:         {}ms", metrics.build_time_ms);
            println!("  enhance:       {}ms", metrics.enhance_time_ms);
            println!("  nodes:         {}", metrics.nodes_processed);
            println!("  summaries:     {}", metrics.summaries_generated);
            println!("  llm calls:     {}", metrics.llm_calls);
            println!("  tokens:        {}", metrics.total_tokens_generated);
            println!("  topics:        {}", metrics.topics_indexed);
            println!("  keywords:      {}", metrics.keywords_indexed);
        }
    }

    for fail in &result.failed {
        eprintln!("FAILED: {} — {}", fail.source, fail.error);
    }

    // Cleanup workspace
    for doc in engine.list().await? {
        engine.remove(&doc.id).await?;
    }

    Ok(())
}
