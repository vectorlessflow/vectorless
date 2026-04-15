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
//! # Using environment variables for LLM config (overrides config file):
//! LLM_API_KEY=sk-xxx LLM_MODEL=gpt-4o cargo run --example advanced
//!
//! # Or with defaults (using config file):
//! cargo run --example advanced
//! ```

use vectorless::{EngineBuilder, IndexContext, QueryContext};

#[tokio::main]
async fn main() -> vectorless::Result<()> {
    // Initialize tracing for debug output (set RUST_LOG=debug to see more)
    tracing_subscriber::fmt::init();

    println!("=== Vectorless Advanced Example (Config File) ===\n");

    // Load all settings from the specified config file.
    // The config file must include api_key and model.
    // If environment variables are set, they override the config file values.
    let mut builder = EngineBuilder::new().with_config_path("./config.toml");

    // Override config with env vars if present
    if let Ok(api_key) = std::env::var("LLM_API_KEY") {
        builder = builder.with_key(&api_key);
    }
    if let Ok(model) = std::env::var("LLM_MODEL") {
        builder = builder.with_model(&model);
    }
    if let Ok(endpoint) = std::env::var("LLM_ENDPOINT") {
        builder = builder.with_endpoint(&endpoint);
    }

    let client = builder
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
        .query(QueryContext::new("What features does Vectorless provide?").with_doc_ids(vec![doc_id.clone()]))
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
