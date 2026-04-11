// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Advanced usage example - Full Configuration.
//!
//! This example demonstrates how to use a configuration file
//! for advanced use cases where you need fine-grained control.
//!
//! # Usage
//!
//! ```bash
//! # First, copy the example config and edit it
//! cp config.toml ./my_vectorless.toml
//! # Edit my_vectorless.toml to customize settings
//!
//! cargo run --example advanced
//! ```

use vectorless::{EngineBuilder, IndexContext, QueryContext};

#[tokio::main]
async fn main() -> vectorless::Result<()> {
    println!("=== Vectorless Advanced Example (Config File) ===\n");

    // Load all settings from the specified config file.
    // The config file must include api_key and model.
    let client = EngineBuilder::new()
        .with_config_path("./config.toml")
        .build()
        .await
        .map_err(|e: vectorless::BuildError| vectorless::Error::Config(e.to_string()))?;

    println!("Client created with config file\n");

    // Index a document
    let result = client.index(IndexContext::from_path("./README.md")).await?;
    let doc_id = result.doc_id().unwrap().to_string();
    println!("Indexed: {}\n", doc_id);

    // Query
    let result = client
        .query(QueryContext::new("What features does Vectorless provide?").with_doc_id(&doc_id))
        .await?;
    println!("Query: What features does Vectorless provide?");
    if let Some(item) = result.single() {
        println!("Score: {:.2}", item.score);
        if !item.content.is_empty() {
            let preview: String = item.content.chars().take(200).collect();
            println!("Result: {}...\n", preview);
        }
    }

    // Cleanup
    client.remove(&doc_id).await?;
    println!("Cleaned up");

    println!("\n=== Done ===");
    Ok(())
}
