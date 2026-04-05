# Quick Start Guide

Get up and running with Vectorless in 5 minutes.

## Prerequisites

- Rust 1.70+ installed
- An OpenAI API key (or compatible LLM endpoint)

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
vectorless = "0.1"
tokio = { version = "1", features = ["full"] }
```

## Basic Usage

```rust
use vectorless::{Engine, IndexContext};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Create an engine with OpenAI
    let engine = Engine::builder()
        .with_workspace("./workspace")
        .with_openai(std::env::var("OPENAI_API_KEY")?)
        .build()
        .await?;

    // 2. Index a document
    let doc_id = engine.index(IndexContext::from_path("./manual.md")).await?;
    println!("Indexed: {}", doc_id);

    // 3. Query the document
    let result = engine.query(&doc_id, "How do I configure authentication?").await?;
    println!("Answer: {}", result.content);

    Ok(())
}
```

## Index from Different Sources

```rust
// From file path
let id1 = engine.index(IndexContext::from_path("./doc.pdf")).await?;

// From string content
let html = "<html><body><h1>Title</h1><p>Content</p></body></html>";
let id2 = engine.index(
    IndexContext::from_content(html, vectorless::parser::DocumentFormat::Html)
        .with_name("webpage")
).await?;

// From bytes (e.g., from HTTP response)
let pdf_bytes = std::fs::read("./document.pdf")?;
let id3 = engine.index(
    IndexContext::from_bytes(pdf_bytes, vectorless::parser::DocumentFormat::Pdf)
).await?;
```

## Index Modes

```rust
use vectorless::IndexMode;

// Default: Skip if already indexed
engine.index(IndexContext::from_path("./doc.md")).await?;

// Force: Always re-index
engine.index(
    IndexContext::from_path("./doc.md").with_mode(IndexMode::Force)
).await?;

// Incremental: Only re-index if changed
engine.index(
    IndexContext::from_path("./doc.md").with_mode(IndexMode::Incremental)
).await?;
```

## Next Steps

- [Understanding the Dual Pipeline](./dual-pipeline.md) - Learn how Vectorless works
- [Indexing Documents](./indexing.md) - Deep dive into document indexing
- [Querying Documents](./querying.md) - Advanced query techniques
