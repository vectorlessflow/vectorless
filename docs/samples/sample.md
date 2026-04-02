
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
