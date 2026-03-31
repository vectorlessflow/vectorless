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

use vectorless::VectorlessBuilder;
use vectorless::client::IndexOptions;

/// Sample markdown content for demonstration.
const SAMPLE_MARKDOWN: &str = r#"
# Project Documentation

This document describes the architecture and usage of the vectorless library.

## Overview

Vectorless is a document indexing and retrieval library that uses tree-based navigation instead of vector embeddings.

### Key Features

- **Tree-Based Indexing** — Documents are organized as hierarchical trees
- **LLM Navigation** — Intelligent traversal using LLM to find relevant content
- **No Vector Database** — Eliminates infrastructure complexity

## Architecture

The crate is organized into several modules:

### Core Module

The core module provides fundamental types:
- `DocumentTree` — Arena-based tree structure
- `TreeNode` — A node in the document tree
- `NodeId` — Unique identifier for tree nodes

### Parser Module

The parser module handles document parsing:
- `MarkdownParser` — Parse Markdown files
- `PdfParser` — Parse PDF files (planned)
- `HtmlParser` — Parse HTML files (planned)

## Usage Examples

### Basic Usage

```rust
use vectorless::client::{Vectorless, VectorlessBuilder};

let client = VectorlessBuilder::new()
    .with_workspace("./workspace")
    .build()?;

let doc_id = client.index("./document.md").await?;
```

### Advanced Usage

You can customize the retrieval process:

```rust
use vectorless::{LlmNavigator, RetrieveOptions};

let retriever = LlmNavigator::with_defaults();
let options = RetrieveOptions::new()
    .with_top_k(5)
    .with_min_score(0.5);

let results = retriever.retrieve(&tree, "What is vectorless?", &options).await?;
```

## Configuration

The library can be configured via TOML files or programmatically.

### Configuration File

```toml
[summary]
model = "gpt-4"
max_tokens = 200

[retrieval]
model = "gpt-4"
top_k = 3
```

## API Reference

See the API documentation for detailed information about each function and type.
"#;

#[tokio::main]
async fn main() -> vectorless::core::Result<()> {
    println!("=== Vectorless Markdown Flow Example ===\n");

    // Step 1: Create a Vectorless client (no API key needed - LLM config is automatic)
    println!("Step 1: Creating Vectorless client...");

    let mut client = VectorlessBuilder::new()
        .build()
        .map_err(|e| vectorless::core::Error::Config(e.to_string()))?;

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
    let options = IndexOptions::new().with_summaries();
    let doc_id = client.index_with_options(&md_path, options).await?;

    println!("  - Document indexed successfully");
    println!("  - Document ID: {}", doc_id);
    println!();

    // Step 3: Show document structure in JSON format
    println!("Step 3: Document structure (JSON):");
    println!();

    match client.get_structure(&doc_id) {
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

    let queries = vec![
        "What are the key features?",
        "How do I configure the library?",
        "What modules are available?",
    ];

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

    client.remove(&doc_id)?;
    println!("  - Document removed");

    println!("\n=== Example Complete ===");
    Ok(())
}
