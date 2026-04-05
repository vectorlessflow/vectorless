// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Complete Markdown processing flow example.
//!
//! This example demonstrates the full pipeline:
//! 1. Create a Vectorless client
//! 2. Index a Markdown document
//! 3. Show document structure in JSON format
//! 4. Query the document
//!
//! # Usage
//!
//! ```bash
//! # Without summaries (default)
//! cargo run --example markdown_flow
//!
//! # With summary generation (requires OPENAI_API_KEY)
//! OPENAI_API_KEY=sk-... cargo run --example markdown_flow
//! ```

use vectorless::Engine;
use vectorless::client::{IndexContext, IndexOptions};

/// Sample markdown content for demonstration.
const SAMPLE_MARKDOWN: &str = r#"
# Project Documentation

This document describes the architecture and usage of the vectorless library.

## Overview

Vectorless is a document indexing and retrieval library that uses tree-based navigation instead of vector embeddings.
"#;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing for debug output (set RUST_LOG=debug to see more)
    tracing_subscriber::fmt::init();

    println!("=== Vectorless Markdown Flow Example ===\n");

    // Step 1: Create a Vectorless client (no API key needed - LLM config is automatic)
    println!("Step 1: Creating Vectorless client...");

    let client = Engine::builder()
        .with_workspace("./workspace")
        .build()
        .await
        .map_err(|e: vectorless::BuildError| vectorless::Error::Config(e.to_string()))?;

    println!("  - Client created successfully");
    println!();

    // Step 2: Index the sample Markdown document
    println!("Step 2: Indexing Markdown document...");

    // Write sample content to a temp file
    let temp_dir = tempfile::tempdir()?;
    let md_path = temp_dir.path().join("sample.md");
    tokio::fs::write(&md_path, SAMPLE_MARKDOWN).await?;

    // Check if we should generate summaries (requires API key)
    println!("  - API key detected, generating summaries...");
    let doc_id = client.index(
        IndexContext::from_path(&md_path)
            .with_options(IndexOptions::new().with_summaries())
    ).await?;

    println!("  - Document indexed successfully");
    println!("  - Document ID: {}", doc_id);
    println!();

    // Step 3: Show document structure in JSON format
    println!("Step 3: Document structure (JSON):");
    println!();

    match client.get_structure(&doc_id).await {
        Ok(tree) => {
            // Export to JSON format (PageIndex compatible)
            let structure = tree.to_structure_json("sample.md");
            let json = serde_json::to_string_pretty(&structure)
                .unwrap_or_else(|_| "Failed to serialize".to_string());
            println!("{}", json);
        }
        Err(e) => {
            println!("  - Error getting structure: {}", e);
        }
    }
    println!();

    // Step 4: Query the document
    println!("Step 4: Querying the document...");

    let queries = vec!["What is this project about?"];

    for query in queries {
        println!("  Query: \"{}\"", query);

        match client.query(&doc_id, query).await {
            Ok(result) => {
                if result.content.is_empty() {
                    println!("    - No relevant content found");
                } else {
                    println!("    - Found relevant content:");
                    // Print first 200 chars
                    let preview = if result.content.len() > 200 {
                        format!("{}...", &result.content[..200])
                    } else {
                        result.content.clone()
                    };
                    for line in preview.lines().take(5) {
                        println!("      {}", line);
                    }
                }
            }
            Err(e) => {
                println!("    - Error: {}", e);
            }
        }
        println!();
    }

    // Step 5: Cleanup
    println!("Step 5: Cleanup...");

    client.remove(&doc_id).await?;
    println!("  - Document removed");

    println!("\n=== Example Complete ===");
    Ok(())
}
