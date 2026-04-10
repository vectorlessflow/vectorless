// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Basic usage example for Vectorless.
//!
//! # Usage
//!
//! ```bash
//! cargo run --example basic
//! ```

use vectorless::{EngineBuilder, IndexContext, QueryContext};

#[tokio::main]
async fn main() -> vectorless::Result<()> {
    println!("=== Vectorless Basic Example ===\n");

    // 1. Create an engine
    let engine = EngineBuilder::new()
        .with_workspace("./workspace")
        .build()
        .await
        .map_err(|e: vectorless::BuildError| vectorless::Error::Config(e.to_string()))?;

    println!("Engine created\n");

    // 2. Index a document
    let result = engine.index(IndexContext::from_path("./README.md")).await?;
    let doc_id = result.doc_id().unwrap().to_string();
    println!("Indexed: {}\n", doc_id);

    // 3. List documents
    println!("Documents:");
    for doc in engine.list().await? {
        println!("  - {} ({})", doc.name, doc.id);
    }
    println!();

    // 4. Query
    match engine
        .query(QueryContext::new("What is vectorless?").with_doc_id(&doc_id))
        .await
    {
        Ok(result) => {
            if let Some(item) = result.single() {
                println!("Score: {:.2}", item.score);
                if !item.content.is_empty() {
                    let preview: String = item.content.chars().take(150).collect();
                    println!("Result: {}...", preview);
                }
            }
        }
        Err(e) => println!("Query: {}", e),
    }
    println!();

    // 5. Cleanup
    engine.remove(&doc_id).await?;
    println!("Removed: {}", doc_id);

    println!("\n=== Done ===");
    Ok(())
}
