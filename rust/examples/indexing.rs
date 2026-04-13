// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Batch indexing example — index multiple documents at once.
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

    // Index multiple files from different paths
    let result = engine
        .index(IndexContext::from_paths(&[
            "../README.md",
            "../CLAUDE.md",
            "../LICENSE",
        ]))
        .await?;

    println!("indexed: {}, failed: {}", result.items.len(), result.failed.len());
    for item in &result.items {
        println!("  {} — doc_id: {}", item.name, item.doc_id);
    }
    for fail in &result.failed {
        println!("  FAILED: {} — {}", fail.source, fail.error);
    }

    // Cleanup
    for doc in engine.list().await? {
        engine.remove(&doc.id).await?;
    }

    Ok(())
}