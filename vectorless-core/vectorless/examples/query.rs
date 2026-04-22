// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Query-only example — query an already-indexed document.
//!
//! Assumes the workspace already contains indexed documents
//! (e.g. from `cargo run --example flow` or `index_single`).
//!
//! # Usage
//!
//! ```bash
//! LLM_API_KEY=sk-xxx LLM_MODEL=gpt-4o \
//!   LLM_ENDPOINT=https://api.openai.com/v1 cargo run --example query
//! ```

use vectorless::{EngineBuilder, QueryContext};

#[tokio::main]
async fn main() -> vectorless::Result<()> {
    tracing_subscriber::fmt::init();

    let api_key = std::env::var("LLM_API_KEY").unwrap_or_else(|_| "sk-...".to_string());
    let model = std::env::var("LLM_MODEL").unwrap_or_else(|_| "gpt-4o".to_string());
    let endpoint = std::env::var("LLM_ENDPOINT").unwrap_or_else(|_| "https://api".to_string());

    let engine = EngineBuilder::new()
        .with_key(&api_key)
        .with_model(&model)
        .with_endpoint(&endpoint)
        .build()
        .await
        .map_err(|e| vectorless::Error::Config(e.to_string()))?;

    // List available documents
    let docs = engine.list().await?;
    if docs.is_empty() {
        println!("No indexed documents found. Run an indexing example first.");
        return Ok(());
    }

    println!("Available documents:");
    for doc in &docs {
        println!("  - {} ({})", doc.name, doc.id);
    }
    println!();

    // Query a specific document
    let doc_id = docs[0].id.clone();
    let queries = vec![
        "What is the system architecture?",
        "How does the storage layer work?",
    ];

    for query in queries {
        println!("Query: \"{}\"", query);

        match engine
            .query(QueryContext::new(query).with_doc_ids(vec![doc_id.clone()]))
            .await
        {
            Ok(result) => {
                if let Some(item) = result.single() {
                    if item.content.is_empty() {
                        println!("  No relevant content found");
                    } else {
                        println!("  Found:");
                        for line in item.content.lines() {
                            println!("    {}", line);
                        }
                    }
                } else {
                    println!("  No results");
                }
            }
            Err(e) => {
                println!("  Error: {}", e);
            }
        }
        println!();
    }

    Ok(())
}
