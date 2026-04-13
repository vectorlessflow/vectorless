// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Batch indexing example — index multiple documents via the vectorless engine.
//!
//! ```bash
//! # Using environment variables for LLM config:
//! LLM_API_KEY=sk-xxx LLM_MODEL=google/gemini-3-flash-preview \
//!   LLM_ENDPOINT=http://localhost:4000/api/v1 cargo run --example indexing
//!
//! # Or with defaults (edit the code to set your key/endpoint):
//! cargo run --example indexing
//! ```

use vectorless::{EngineBuilder, IndexContext};

#[tokio::main]
async fn main() -> vectorless::Result<()> {
    // Initialize tracing for debug output (set RUST_LOG=debug to see more)
    tracing_subscriber::fmt::init();

    // Build engine with LLM configuration from environment or defaults.
    // Adjust the defaults below to match your setup.
    let api_key = std::env::var("LLM_API_KEY")
        .unwrap_or_else(|_| "sk-or-v1-...".to_string());
    let model = std::env::var("LLM_MODEL")
        .unwrap_or_else(|_| "google/gemini-3-flash-preview".to_string());
    let endpoint = std::env::var("LLM_ENDPOINT")
        .unwrap_or_else(|_| "http://localhost:4000/api/v1".to_string());

    let engine = EngineBuilder::new()
        .with_workspace("./workspace_batch_example")
        .with_key(&api_key)
        .with_model(&model)
        .with_endpoint(&endpoint)
        .build()
        .await
        .map_err(|e| vectorless::Error::Config(e.to_string()))?;

    // Index multiple documents in a single call.
    // Paths are resolved relative to the workspace directory.
    let result = engine
        .index(
            IndexContext::from_paths(&["../README.md", "../CLAUDE.md"]))
        .await?;

    println!("Indexed {} document(s)", result.items.len());
    for item in &result.items {
        println!("  - {} ({})", item.name, item.doc_id);
        if let Some(metrics) = &item.metrics {
            println!("    Time: {}ms", metrics.total_time_ms());
            println!("    Nodes: {}", metrics.nodes_processed);
        }
    }

    // Cleanup
    for doc in engine.list().await? {
        engine.remove(&doc.id).await?;
    }

    Ok(())
}