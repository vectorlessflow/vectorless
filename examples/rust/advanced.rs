// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Advanced usage example - Full Configuration.
//!
//! This example demonstrates how to use a full configuration file
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
    println!("=== Vectorless Advanced Example (Full Configuration) ===\n");

    // Method 1: Use explicit config file path
    // This loads all settings from the specified config file
    let client = EngineBuilder::new()
        .with_config_path("./config.toml") // or "./my_vectorless.toml"
        .build()
        .await
        .map_err(|e: vectorless::BuildError| vectorless::Error::Config(e.to_string()))?;

    println!("✓ Client created with config file\n");

    // Index a document
    let result = client.index(IndexContext::from_path("./README.md")).await?;
    let doc_id = result.doc_id().unwrap().to_string();
    println!("✓ Indexed: {}\n", doc_id);

    // Query
    let result = client
        .query(QueryContext::new("What features does Vectorless provide?").with_doc_id(&doc_id))
        .await?;
    println!("Query: What features does Vectorless provide?");
    println!("Score: {:.2}", result.score);
    if !result.content.is_empty() {
        let preview: String = result.content.chars().take(200).collect();
        println!("Result: {}...\n", preview);
    }

    // Cleanup
    client.remove(&doc_id).await?;
    println!("✓ Cleaned up");

    println!("\n=== Configuration Options ===\n");
    println!("Configuration Priority (later overrides earlier):");
    println!("  1. Default configuration");
    println!("  2. Auto-detected config file (vectorless.toml, config.toml, .vectorless.toml)");
    println!("  3. Explicit config file (with_config_path)");
    println!("  4. Environment variables (OPENAI_API_KEY, VECTORLESS_MODEL, etc.)");
    println!("  5. Builder methods (with_key, with_model, with_endpoint)");
    println!();
    println!("Environment Variables:");
    println!("  OPENAI_API_KEY       - LLM API key");
    println!("  VECTORLESS_MODEL     - Default model name");
    println!("  VECTORLESS_ENDPOINT  - API endpoint URL");
    println!("  VECTORLESS_WORKSPACE - Workspace directory");

    println!("\n=== Done ===");
    Ok(())
}
