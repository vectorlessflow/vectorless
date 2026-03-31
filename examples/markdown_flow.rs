// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Complete Markdown processing flow example.
//!
//! This example demonstrates the full pipeline:
//! 1. Parse a Markdown document
//! 2. Build a document tree
//! 3. Retrieve relevant content (simulated)
//! 4. Score and rank results
//! 5. Build final context
//!
//! # Usage
//!
//! ```bash
//! cargo run --example markdown_flow
//! ```

use vectorless::{
    DocumentParser, MarkdownParser,
    TreeBuilder,
    RetrieveOptions, RetrievalResult, Retriever,
    Scorer, Merger, ScoringStrategy, MergeStrategy,
    ContextBuilder,
    LlmNavigator,
};

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
    .with_top_k(5);

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
    println!("=== Markdown Processing Flow Example ===\n");

    // Step 1: Parse the Markdown document
    println!("Step 1: Parsing Markdown document...");
    let parser = MarkdownParser::new();
    let parse_result = parser.parse(SAMPLE_MARKDOWN).await?;

    println!("  - Document name: {}", parse_result.meta.name);
    println!("  - Nodes extracted: {}", parse_result.nodes.len());
    println!();

    // Step 2: Build the document tree
    println!("Step 2: Building document tree...");
    let tree_builder = TreeBuilder::new()
        .with_root_title("Project Documentation")
        .with_root_content("This document describes the vectorless library.");

    let tree = tree_builder.build_with_ids(parse_result.nodes);

    println!("  - Total nodes: {}", tree.node_count());

    // Print tree structure
    fn print_tree(tree: &vectorless::DocumentTree, node_id: vectorless::NodeId, indent: usize) {
        if let Some(node) = tree.get(node_id) {
            let prefix = "  ".repeat(indent);
            println!("{}- {} (depth: {})", prefix, node.title, node.depth);
            for child_id in tree.children(node_id) {
                print_tree(tree, child_id, indent + 1);
            }
        }
    }

    println!("  - Tree structure:");
    print_tree(&tree, tree.root(), 2);
    println!();

    // Step 3: Retrieve relevant content
    println!("Step 3: Retrieving relevant content...");
    let retriever = LlmNavigator::with_defaults();
    let options = RetrieveOptions::new()
        .with_top_k(3);

    let query = "What are the key features of vectorless?";
    println!("  - Query: \"{}\"", query);

    let results: Vec<String> = retriever.retrieve(&tree, query, &options).await?;
    println!("  - Results found: {}", results.len());
    println!();

    // Step 4: Convert to RetrievalResult format for scoring
    println!("Step 4: Converting results for scoring...");

    let retrieval_results: Vec<RetrievalResult> = results.iter()
        .filter_map(|content: &String| {
            let lines: Vec<&str> = content.lines().collect();
            if lines.is_empty() {
                return None;
            }
            let title = lines[0].trim_start_matches('#').trim().to_string();
            Some(RetrievalResult::new(title)
                .with_content(content.clone())
                .with_score(0.5))
        })
        .collect();

    println!("  - Converted {} results", retrieval_results.len());
    println!();

    // Step 5: Score and rank results
    println!("Step 5: Scoring and ranking results...");

    let scorer = Scorer::new()
        .with_strategy(ScoringStrategy::Combined)
        .with_tf_weight(0.3)
        .with_position_weight(0.2);

    let scored = scorer.score(&retrieval_results, query);
    println!("  - Scored {} results", scored.len());

    for (i, result) in scored.iter().enumerate() {
        println!("    {}. {} (score: {:.3})", i + 1, result.result.title, result.score);
    }
    println!();

    // Step 6: Merge and deduplicate
    println!("Step 6: Merging and deduplicating...");
    let merger = Merger::new()
        .with_strategy(MergeStrategy::DeduplicateTitle)
        .with_min_score(0.3)
        .with_max_results(5);

    let merged = merger.merge(scored);
    println!("  - Merged results: {}", merged.len());

    for (i, result) in merged.iter().enumerate() {
        println!("    {}. {} (score: {:.3})", i + 1, result.result.title, result.score);
    }
    println!();

    // Step 7: Build final context
    println!("Step 7: Building final context...");

    // Extract RetrievalResult from ScoredResult
    let final_results: Vec<RetrievalResult> = merged.into_iter()
        .map(|s| s.result)
        .collect();

    let context_builder = ContextBuilder::new()
        .with_max_tokens(2000)
        .with_titles(true)
        .with_content(true);

    let context = context_builder.build(&final_results);
    println!("  - Context length: {} characters", context.len());
    println!();

    println!("=== Final Context ===");
    println!("{}", context);

    println!("\n=== Example Complete ===");
    Ok(())
}
