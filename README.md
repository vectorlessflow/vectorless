<div align="center">

<img src="https://raw.githubusercontent.com/vectorlessflow/vectorless/main/docs/design/with-title.png" alt="Vectorless" width="400" style="vertical-align:middle;">

<h1>Reasoning-native Document Intelligence Engine</h1>

[![PyPI](https://img.shields.io/pypi/v/vectorless.svg)](https://pypi.org/project/vectorless/)
[![PyPI Downloads](https://static.pepy.tech/badge/vectorless/month)](https://pepy.tech/projects/vectorless)
[![Crates.io](https://img.shields.io/crates/v/vectorless.svg)](https://crates.io/crates/vectorless)
[![Crates.io Downloads](https://img.shields.io/crates/d/vectorless.svg)](https://crates.io/crates/vectorless)
[![Docs](https://docs.rs/vectorless/badge.svg)](https://docs.rs/vectorless)
[![License](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE)

</div>

**Vectorless** is an ultra-performant reasoning-native document intelligence engine for AI, with the core written in Rust. It transforms documents into rich semantic trees and uses LLMs to intelligently traverse the hierarchy — retrieving the most relevant content through structural reasoning and deep contextual understanding.

<div align="center"><img src="https://raw.githubusercontent.com/vectorlessflow/vectorless/main/docs/design/positioning.svg" alt="Vectorless" width="550" height="550"></div>


## Quick Start

### Install

```bash
pip install vectorless
```

### Set your API key

```bash
export OPENAI_API_KEY="sk-..."
```

### Index and Query

```python
from vectorless import Engine, IndexContext

# Create engine with a workspace directory
engine = Engine(workspace="./data")

# Index a document (PDF, Markdown, DOCX, HTML)
result = engine.index(IndexContext.from_file("./report.pdf"))
doc_id = result.doc_id

# Query
result = engine.query(doc_id, "What is the total revenue?")
print(result.content)
print(f"Score: {result.score}")
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
        .with_workspace("./data")
        .build()
        .await?;

    // Index
    let result = engine.index(IndexContext::from_path("./report.pdf")).await?;
    let doc_id = result.doc_id().unwrap();

    // Query
    let result = engine.query(
        QueryContext::new("What is the total revenue?").with_doc_id(doc_id)
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
