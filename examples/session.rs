// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Session-based multi-document operations example.
//!
//! This example demonstrates the Session API for:
//! - Managing multiple documents in a single session
//! - Cross-document queries
//! - Session caching for improved performance
//! - Session statistics
//!
//! # Usage
//!
//! ```bash
//! cargo run --example session
//! ```

use vectorless::client::{EngineBuilder, IndexContext};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Session-Based Multi-Document Example ===\n");

    // 1. Create the engine
    println!("Step 1: Creating engine...");
    let engine = EngineBuilder::new()
        .with_workspace("./workspace_session_example")
        .build()
        .await
        .map_err(|e: vectorless::BuildError| vectorless::Error::Config(e.to_string()))?;
    println!("  ✓ Engine created\n");

    // 2. Create a session for multi-document operations
    println!("Step 2: Creating session...");
    let session = engine.session().await;
    println!("  ✓ Session ID: {}\n", session.id());

    // 3. Index multiple documents into the session
    println!("Step 3: Indexing documents...");

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
- `query(doc_id, question)`: Query a document
- `list_documents()`: List all documents

## Session

Multi-document operations with caching.

### Methods

- `index(path)`: Index into session
- `query(doc_id, question)`: Query cached document
- `query_all(question)`: Query across all documents
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

    // Index into session
    let doc1_id = session.index(IndexContext::from_path(&doc1_path)).await?;
    println!("  ✓ Indexed: architecture.md -> {}", &doc1_id[..8]);

    let doc2_id = session.index(IndexContext::from_path(&doc2_path)).await?;
    println!("  ✓ Indexed: api.md -> {}", &doc2_id[..8]);

    let doc3_id = session.index(IndexContext::from_path(&doc3_path)).await?;
    println!("  ✓ Indexed: config.md -> {}", &doc3_id[..8]);
    println!();

    // 4. List documents in session
    println!("Step 4: Session documents:");
    for doc in session.list_documents() {
        println!("  - {} ({})", doc.name, &doc.id[..8]);
    }
    println!();

    // 5. Query individual documents (uses cache)
    println!("Step 5: Query individual documents...");
    let query = "What methods are available?";

    println!("  Query: \"{}\"", query);
    let start = std::time::Instant::now();
    let result = session.query(&doc2_id, query).await?;
    let elapsed = start.elapsed();
    println!("    - Time: {:?}", elapsed);
    println!("    - Score: {:.2}", result.score);
    if !result.content.is_empty() {
        let preview: String = result.content.chars().take(100).collect();
        println!("    - Preview: {}...", preview);
    }
    println!();

    // 6. Query same document again (should be faster due to cache)
    println!("Step 6: Query cached document (should be faster)...");
    let start = std::time::Instant::now();
    let result = session.query(&doc2_id, "How to list documents?").await?;
    let cached_elapsed = start.elapsed();
    println!("    - Time: {:?}", cached_elapsed);
    println!("    - Score: {:.2}", result.score);
    println!();

    // 7. Query across all documents
    println!("Step 7: Cross-document query...");
    let query = "How to configure the workspace?";
    println!("  Query: \"{}\"", query);

    let results = session.query_all(query).await?;
    println!("  Found {} relevant documents:", results.len());

    for (i, result) in results.iter().enumerate() {
        println!(
            "    {}. {} (score: {:.2})",
            i + 1,
            &result.doc_id[..8],
            result.score
        );
    }
    println!();

    // 8. Show session statistics
    println!("Step 8: Session statistics:");
    let stats = session.stats();
    println!("  - Documents: {}", session.list_documents().len());
    println!("  - Queries: {}", stats.query_count.get());
    println!("  - Cache hits: {}", stats.cache_hits.get());
    println!("  - Cache misses: {}", stats.cache_misses.get());
    println!("  - Cache hit rate: {:.1}%", stats.cache_hit_rate() * 100.0);
    if let Some(avg_time) = stats.avg_query_time() {
        println!("  - Avg query time: {:?}", avg_time);
    }
    println!("  - Session age: {:?}", session.age());
    println!();

    // 9. Cleanup
    println!("Step 9: Cleanup...");
    engine.remove(&doc1_id).await?;
    engine.remove(&doc2_id).await?;
    engine.remove(&doc3_id).await?;
    println!("  ✓ Documents removed\n");

    println!("=== Example Complete ===");
    Ok(())
}
