# Vectorless

[![Crates.io](https://img.shields.io/crates/v/vectorless.svg)](https://crates.io/crates/vectorless)
[![Downloads](https://img.shields.io/crates/d/vectorless.svg)](https://crates.io/crates/vectorless)
[![Documentation](https://docs.rs/vectorless/badge.svg)](https://docs.rs/vectorless)
[![License](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.85%2B-orange.svg)](https://www.rust-lang.org/)

A hierarchical, reasoning-native document intelligence engine.

> ⭐ **Star us on [GitHub](https://github.com/vectorlessflow/vectorless)** — it helps the project grow!

## Features

- **Tree-based indexing** — Documents are organized as hierarchical trees, not flat vectors
- **LLM-driven retrieval** — Uses reasoning to navigate document structure
- **Multi-format support** — Markdown, PDF, HTML, DOCX (planned)
- **Workspace persistence** — LRU-cached storage with lazy loading
- **Configurable retrieval** — Pluggable retriever strategies (LLM navigate, beam search, MCTS)

## Quick Start

```rust
use vectorless::client::{Vectorless, VectorlessBuilder};

#[tokio::main]
async fn main() -> vectorless::core::Result<()> {
    // Create client
    let mut client = VectorlessBuilder::new()
        .with_workspace("./workspace")
        .build()?;

    // Index a document
    let doc_id = client.index("./document.md").await?;

    // Query
    let result = client.query(&doc_id, "What is this about?").await?;
    println!("{}", result.content);

    Ok(())
}
```

## Configuration

Create `config.toml` in your project root:

```toml
[summary]
model = "gpt-4o-mini"
endpoint = "https://api.openai.com/v1"
api_key = "sk-..."

[retrieval]
model = "gpt-4o"
retriever_type = "llm_navigate"
top_k = 3

[storage]
workspace_dir = "./workspace"
```

## Status

Early development. Core functionality works:
- ✅ Markdown indexing with LLM summaries
- ✅ Tree-based retrieval via LLM navigation
- ✅ Workspace persistence with LRU cache
- 🚧 PDF/HTML/DOCX parsing
- 🚧 Additional retriever strategies

## Contributing

⭐ If you find this project useful, please consider giving it a star on [GitHub](https://github.com/vectorlessflow/vectorless) — it helps others discover it and supports ongoing development.

## License

Apache-2.0
