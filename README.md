# Vectorless

[![Crates.io](https://img.shields.io/crates/v/vectorless.svg)](https://crates.io/crates/vectorless)
[![Downloads](https://img.shields.io/crates/d/vectorless.svg)](https://crates.io/crates/vectorless)
[![Documentation](https://docs.rs/vectorless/badge.svg)](https://docs.rs/vectorless)
[![License](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.85%2B-orange.svg)](https://www.rust-lang.org/)

A hierarchical, reasoning-native document intelligence engine. no vector db, no embeddings. 

> ⭐ **Star us on [GitHub](https://github.com/vectorlessflow/vectorless)** — it helps the project grow!

## Features

- **Tree-based indexing** — Documents organized as hierarchical trees, not flat vectors
- **LLM-driven retrieval** — Uses reasoning to navigate document structure
- **No vector database** — Eliminates embedding infrastructure complexity
- **Multi-format support** — Markdown, PDF (basic), HTML/DOCX (planned)
- **Workspace persistence** — LRU-cached storage with lazy loading
- **Pluggable retrievers** — LLM navigate, beam search, MCTS, hybrid

## Quick Start

* Add to `Cargo.toml`:

```toml
[dependencies]
vectorless = "0.1"
```

* Create `vectorless.toml` in your project root (auto-detected):
```shell
cp config.example.toml vectorless.toml
```

```rust
use vectorless::client::{Vectorless, VectorlessBuilder};

#[tokio::main]
async fn main() -> vectorless::core::Result<()> {
    // Create client (auto-loads config from ./vectorless.toml)
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

## Contributing

⭐ If you find this project useful, please consider giving it a star on [GitHub](https://github.com/vectorlessflow/vectorless) — it helps others discover it and supports ongoing development.

## License

Apache-2.0
