// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Basic usage example for Vectorless.
//!
//! This example demonstrates the core API in ~30 lines.
//!
//! # Usage
//!
//! ```bash
//! cargo run --example basic
//! ```

use vectorless::Engine;

#[tokio::main]
async fn main() -> vectorless::Result<()> {
    println!("=== Vectorless Basic Example ===\n");

    // 1. Create a client
    let client = Engine::builder()
        .with_workspace("./workspace")
        .build()
        .map_err(|e| vectorless::Error::Config(e.to_string()))?;

    println!("✓ Client created\n");

    // 2. Index a document
    let doc_id = client.index("./README.md").await?;
    println!("✓ Indexed: {}\n", doc_id);

    // 3. List documents
    println!("Documents:");
    for doc in client.list_documents() {
        println!("  - {} ({})", doc.name, doc.id);
    }
    println!();

    // 4. Query
    match client.query(&doc_id, "What is vectorless?").await {
        Ok(result) => {
            println!("Query score: {:.2}", result.score);
            if !result.content.is_empty() {
                let preview: String = result.content.chars().take(150).collect();
                println!("Result: {}...", preview);
            }
        }
        Err(e) => println!("Query: {}", e),
    }
    println!();

    // 5. Clone for concurrent use (client is Clone + Send + Sync)
    let _client1 = client.clone();
    let _client2 = client.clone();
    println!("✓ Client cloned for concurrent use\n");

    // 6. Cleanup
    client.remove(&doc_id)?;
    println!("✓ Removed: {}", doc_id);

    println!("\n=== Done ===");
    Ok(())
}
