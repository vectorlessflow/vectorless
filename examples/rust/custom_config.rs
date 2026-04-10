// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Custom configuration example - Using your own API key, model, and endpoint.
//!
//! This example demonstrates how to use custom LLM settings without a config file.
//! Useful when you want to use different providers like Azure OpenAI, DeepSeek, etc.
//!
//! # Usage
//!
//! ```bash
//! cargo run --example custom_config
//! ```

use vectorless::{EngineBuilder, IndexContext, QueryContext};

#[tokio::main]
async fn main() -> vectorless::Result<()> {
    println!("=== Vectorless Custom Configuration Example ===\n");

    // ============================================================
    // Option 1: Use environment variables
    // ============================================================
    // Set these environment variables:
    // - OPENAI_API_KEY or VECTORLESS_API_KEY
    // - VECTORLESS_MODEL (optional, default: gpt-4o)
    // - VECTORLESS_ENDPOINT (optional, default: OpenAI endpoint)
    // - VECTORLESS_WORKSPACE (optional, default: ./workspace)

    // ============================================================
    // Option 2: Use builder methods (recommended for custom config)
    // ============================================================

    // Example: Use DeepSeek API
    let client = EngineBuilder::new()
        .with_workspace("./workspace")
        .with_model("deepseek-chat")
        .with_key("sk-your-deepseek-key")
        .with_endpoint("https://api.deepseek.com/v1")
        .build()
        .await
        .map_err(|e: vectorless::BuildError| vectorless::Error::Config(e.to_string()))?;

    println!("✓ Client created with custom settings\n");

    // Index a document
    let index_result = client.index(IndexContext::from_path("./README.md")).await?;
    let doc_id = index_result.doc_id().unwrap().to_string();
    println!("✓ Indexed: {}\n", doc_id);

    // Query
    let result = client
        .query(QueryContext::new("What is Vectorless?").with_doc_id(&doc_id))
        .await?;
    println!("Query: What is Vectorless?");
    println!("Score: {:.2}", result.score);
    if !result.content.is_empty() {
        let preview: String = result.content.chars().take(200).collect();
        println!("Result: {}...\n", preview);
    }

    // Cleanup
    client.remove(&doc_id).await?;
    println!("✓ Cleaned up");

    // ============================================================
    // Other provider examples (commented out)
    // ============================================================

    // Azure OpenAI:
    // let client = EngineBuilder::new()
    //     .with_workspace("./workspace")
    //     .with_model("gpt-4o")
    //     .with_key("your-azure-key")
    //     .with_endpoint("https://your-resource.openai.azure.com/openai/deployments/your-deployment")
    //     .build()
    //     .await?;

    // Local LLM (e.g., Ollama with OpenAI-compatible API):
    // let client = EngineBuilder::new()
    //     .with_workspace("./workspace")
    //     .with_model("llama3")
    //     .with_endpoint("http://localhost:11434/v1")
    //     .build()
    //     .await?;

    // Anthropic Claude (via OpenAI-compatible proxy):
    // let client = EngineBuilder::new()
    //     .with_workspace("./workspace")
    //     .with_model("claude-3-5-sonnet-20241022")
    //     .with_key("sk-ant-...")
    //     .with_endpoint("https://api.anthropic.com/v1")
    //     .build()
    //     .await?;

    println!("\n=== Done ===");
    Ok(())
}
