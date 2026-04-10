// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Multi-document operations example.
//!
//! This example demonstrates how to use the Engine API for:
//! - Managing multiple documents
//! - Querying individual documents
//! - Tracking document statistics
//!
//! # Usage
//!
//! ```bash
//! cargo run --example session
//! ```

use vectorless::client::{EngineBuilder, IndexContext, QueryContext};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Multi-Document Example ===\n");

    // 1. Create the engine
    println!("Step 1: Creating engine...");
    let engine = EngineBuilder::new()
        .with_workspace("./workspace_session_example")
        .build()
        .await
        .map_err(|e: vectorless::BuildError| vectorless::Error::Config(e.to_string()))?;
    println!("  ✓ Engine created\n");

    // 2. Index multiple documents
    println!("Step 2: Indexing documents...");

    // Create sample documents
    let temp_dir = tempfile::tempdir()?;

    let doc1_content = r#"# Architecture Guide

## Overview

Vectorless uses a tree-based architecture for document navigation.

## Components

- **Indexer**: Parses documents and builds tree structure
- **Retriever**: Navigates tree to find relevant content
- **Workspace**: Manages document persistence
"#;

    let doc2_content = r#"# API Reference

## Engine

The main entry point for vectorless operations.

### Methods

- `index(path)`: Index a document
- `query(question)`: Query a document
- `list()`: List all documents

## Configuration

Custom configuration via builder methods.
"#;

    let doc3_content = r#"# Configuration Guide

## Workspace Settings

The workspace directory stores indexed documents.

```toml
[storage]
workspace_dir = "./workspace"
```

## Retrieval Settings

Configure retrieval behavior:

```toml
[retrieval]
top_k = 5
max_tokens = 4000
```

## Content Aggregator

Control content aggregation:

```toml
[retrieval.content]
enabled = true
token_budget = 4000
```
"#;

    // Write sample documents
    let doc1_path = temp_dir.path().join("architecture.md");
    let doc2_path = temp_dir.path().join("api.md");
    let doc3_path = temp_dir.path().join("config.md");

    tokio::fs::write(&doc1_path, doc1_content).await?;
    tokio::fs::write(&doc2_path, doc2_content).await?;
    tokio::fs::write(&doc3_path, doc3_content).await?;

    // Index documents
    let index1 = engine.index(IndexContext::from_path(&doc1_path)).await?;
    let doc1_id = index1.doc_id().unwrap().to_string();
    println!("  ✓ Indexed: architecture.md -> {}", &doc1_id[..8]);

    let index2 = engine.index(IndexContext::from_path(&doc2_path)).await?;
    let doc2_id = index2.doc_id().unwrap().to_string();
    println!("  ✓ Indexed: api.md -> {}", &doc2_id[..8]);

    let index3 = engine.index(IndexContext::from_path(&doc3_path)).await?;
    let doc3_id = index3.doc_id().unwrap().to_string();
    println!("  ✓ Indexed: config.md -> {}", &doc3_id[..8]);
    println!();

    // 3. List documents
    println!("Step 3: Indexed documents:");
    for doc in engine.list().await? {
        println!("  - {} ({})", doc.name, &doc.id[..8]);
    }
    println!();

    // 4. Query individual documents
    println!("Step 4: Query individual documents...");
    let query = "What methods are available?";

    println!("  Query: \"{}\"", query);
    let start = std::time::Instant::now();
    let result = engine
        .query(QueryContext::new(query).with_doc_id(&doc2_id))
        .await?;
    let elapsed = start.elapsed();
    println!("    - Time: {:?}", elapsed);
    println!("    - Score: {:.2}", result.score);
    if !result.content.is_empty() {
        let preview: String = result.content.chars().take(100).collect();
        println!("    - Preview: {}...", preview);
    }
    println!();

    // 5. Query the same document again
    println!("Step 5: Query another document...");
    let start = std::time::Instant::now();
    let result = engine
        .query(QueryContext::new("How to list documents?").with_doc_id(&doc2_id))
        .await?;
    let elapsed = start.elapsed();
    println!("    - Time: {:?}", elapsed);
    println!("    - Score: {:.2}", result.score);
    println!();

    // 6. Query each document individually
    println!("Step 6: Query each document...");
    let query = "How to configure the workspace?";
    println!("  Query: \"{}\"", query);

    let doc_ids = vec![
        ("architecture.md", &doc1_id),
        ("api.md", &doc2_id),
        ("config.md", &doc3_id),
    ];

    for (name, id) in &doc_ids {
        match engine.query(QueryContext::new(query).with_doc_id(&**id)).await {
            Ok(result) => {
                println!(
                    "    - {} (score: {:.2})",
                    name, result.score
                );
            }
            Err(e) => {
                println!("    - {} Error: {}", name, e);
            }
        }
    }
    println!();

    // 7. Show document count
    println!("Step 7: Document statistics:");
    let docs = engine.list().await?;
    println!("  - Total documents: {}", docs.len());
    println!();

    // 8. Cleanup
    println!("Step 8: Cleanup...");
    engine.remove(&doc1_id).await?;
    engine.remove(&doc2_id).await?;
    engine.remove(&doc3_id).await?;
    println!("  ✓ Documents removed\n");

    println!("=== Example Complete ===");
    Ok(())
}
