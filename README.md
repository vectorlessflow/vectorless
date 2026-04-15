<div align="center">

<img src="https://vectorless.dev/img/with-title.png" alt="Vectorless" width="400" style="vertical-align:middle;">

<h1>Reasoning-native Document Intelligence Engine</h1>

[![PyPI](https://img.shields.io/pypi/v/vectorless.svg)](https://pypi.org/project/vectorless/)
[![PyPI Downloads](https://static.pepy.tech/badge/vectorless/month)](https://pepy.tech/projects/vectorless)
[![Crates.io](https://img.shields.io/crates/v/vectorless.svg)](https://crates.io/crates/vectorless)
[![Crates.io Downloads](https://img.shields.io/crates/d/vectorless.svg)](https://crates.io/crates/vectorless)
[![Docs](https://docs.rs/vectorless/badge.svg)](https://docs.rs/vectorless)
[![License](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE)

</div>

**Vectorless** is a reasoning-native document intelligence engine written in Rust — **no vector database, no embeddings, no similarity search**. It transforms documents into hierarchical semantic trees and uses LLMs to navigate the structure, retrieving the most relevant content through deep contextual understanding instead of vector math.


## Quick Start

### Install

```bash
pip install vectorless
```

### Index and Query

```python
import asyncio
from vectorless import Engine, IndexContext

async def main():
    # Create engine — api_key and model are required
    engine = Engine(
        api_key="sk-...",
        model="gpt-4o",
    )

    # Index a document (PDF or Markdown)
    result = await engine.index(IndexContext.from_file("./report.pdf"))
    doc_id = result.doc_id

    # Query
    result = await engine.query(doc_id, "What is the total revenue?")
    print(result.single().content)

asyncio.run(main())
```

<details>
<summary><b>Rust</b></summary>

```toml
[dependencies]
vectorless = "0.1"
```

```rust
use vectorless::client::{EngineBuilder, IndexContext, QueryContext};

#[tokio::main]
async fn main() -> vectorless::Result<()> {
    let engine = EngineBuilder::new()
        .with_key("sk-...")
        .with_model("gpt-4o")
        .build()
        .await?;

    // Index
    let result = engine.index(IndexContext::from_path("./report.pdf")).await?;
    let doc_id = result.doc_id().unwrap();

    // Query
    let result = engine.query(
        QueryContext::new("What is the total revenue?").with_doc_ids(vec![doc_id.to_string()])
    ).await?;
    println!("Answer: {}", result.content);

    Ok(())
}
```
</details>

## Examples

See [examples](examples/) for more and stay tuned.

## Contributing

Contributions welcome! If you find this useful, please ⭐ the repo — it helps others discover it.

## Star History

<a href="https://www.star-history.com/?repos=vectorlessflow%2Fvectorless&type=date&legend=top-left">
 <picture>
   <source media="(prefers-color-scheme: dark)" srcset="https://api.star-history.com/chart?repos=vectorlessflow/vectorless&type=date&theme=dark&legend=top-left" />
   <source media="(prefers-color-scheme: light)" srcset="https://api.star-history.com/chart?repos=vectorlessflow/vectorless&type=date&legend=top-left" />
   <img alt="Star History Chart" src="https://api.star-history.com/chart?repos=vectorlessflow/vectorless&type=date&legend=top-left" />
 </picture>
</a>

## License

Apache License 2.0
