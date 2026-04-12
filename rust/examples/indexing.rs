// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Batch indexing example — index multiple documents at once.
//!
//! ```bash
//! cargo run --example indexing
//! ```

use vectorless::{EngineBuilder, IndexContext};

#[tokio::main]
async fn main() -> vectorless::Result<()> {
    let engine = EngineBuilder::new()
        .with_workspace("./workspace_batch_example")
        .with_key("sk-or-v1-...")
        .with_model("google/gemini-3-flash-preview")
        .with_endpoint("http://localhost:4000/api/v1")
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
