// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Incremental indexing example — re-index with change detection.
//!
//! ```bash
//! cargo run --example index_incremental
//! ```

use vectorless::{DocumentFormat, EngineBuilder, IndexContext, IndexMode};

#[tokio::main]
async fn main() -> vectorless::Result<()> {
    let engine = EngineBuilder::new()
        .with_workspace("./workspace_incremental_example")
        .with_key("sk-or-v1-...")
        .with_model("google/gemini-3-flash-preview")
        .with_endpoint("http://localhost:4000/api/v1")
        .build()
        .await
        .map_err(|e| vectorless::Error::Config(e.to_string()))?;

    let content_v1 = r#"# API Reference

## GET /users

Returns a list of all users in the system.

## POST /users

Creates a new user account.
"#;

    let content_v2 = r#"# API Reference

## GET /users

Returns a paginated list of users. Supports `?page=` and `?limit=` parameters.

## POST /users

Creates a new user account. Requires email and password fields.

## DELETE /users/:id

Deletes a user by their unique identifier.
"#;

    // 1. Initial full index
    println!("--- Initial index ---");
    let result = engine
        .index(IndexContext::from_content(content_v1, DocumentFormat::Markdown))
        .await?;

    let doc_id = result.items[0].doc_id.clone();
    if let Some(m) = &result.items[0].metrics {
        println!("indexed in {}ms, {} nodes", m.total_time_ms(), m.nodes_processed);
    }

    // 2. Re-index unchanged content (incremental) — skips processing
    println!("\n--- Re-index unchanged (incremental) ---");
    let result = engine
        .index(
            IndexContext::from_content(content_v1, DocumentFormat::Markdown)
                .with_mode(IndexMode::Incremental),
        )
        .await?;

    for item in &result.items {
        println!("doc_id: {} (unchanged, skipped)", item.doc_id);
    }

    // 3. Re-index with changes (incremental) — detects diff and updates
    println!("\n--- Re-index with changes (incremental) ---");
    let result = engine
        .index(
            IndexContext::from_content(content_v2, DocumentFormat::Markdown)
                .with_mode(IndexMode::Incremental),
        )
        .await?;

    for item in &result.items {
        if let Some(m) = &item.metrics {
            println!("updated in {}ms, {} nodes", m.total_time_ms(), m.nodes_processed);
        }
    }

    println!("\ndoc_id: {doc_id}");

    // Cleanup
    for doc in engine.list().await? {
        engine.remove(&doc.id).await?;
    }

    Ok(())
}
