# Vectorless

A hierarchical, reasoning-native document intelligence engine.

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

## License

Apache-2.0
