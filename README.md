<div align="center">

<img src="https://raw.githubusercontent.com/vectorlessflow/vectorless/main/docs/design/logo-horizontal.svg" alt="Vectorless">

<h1>Reasoning-native document inteligence engine</h1>


[![PyPI](https://img.shields.io/pypi/v/vectorless.svg)](https://pypi.org/project/vectorless/)
[![Python](https://img.shields.io/pypi/pyversions/vectorless.svg)](https://pypi.org/project/vectorless/)
[![PyPI Downloads](https://static.pepy.tech/badge/vectorless/month)](https://pepy.tech/projects/vectorless)
[![Crates.io](https://img.shields.io/crates/v/vectorless.svg)](https://crates.io/crates/vectorless)
[![Crates.io Downloads](https://img.shields.io/crates/d/vectorless.svg)](https://crates.io/crates/vectorless)
[![Docs](https://docs.rs/vectorless/badge.svg)](https://docs.rs/vectorless)
[![License](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.85%2B-orange.svg)](https://www.rust-lang.org/)

</div>

**Vectorless** is a library for querying structured documents using natural language — without vector databases or embedding models. Core engine written in **Rust**, with Python bindings.
Instead of chunking documents into vectors, Vectorless preserves the document's tree structure and uses LLM to navigate it — like how a human reads a table of contents.

⭐ Drop a star to help us grow!

## How It Works

<img src="https://raw.githubusercontent.com/vectorlessflow/vectorless/main/docs/design/how-it-works.svg" alt="How it works">

### 1. Index: Build a Navigable Tree

```
Technical Manual (root)
├── Chapter 1: Introduction
├── Chapter 2: Architecture
│   ├── 2.1 System Design
│   └── 2.2 Implementation
└── Chapter 3: API Reference
```

Each node gets an AI-generated summary, enabling fast navigation.

### 2. Query: Navigate with LLM

When you ask "How do I reset the device?":

1. **Analyze** — Understand query intent and complexity
2. **Navigate** — LLM guides tree traversal
3. **Retrieve** — Return the exact section with context
4. **Verify** — Check if more information is needed

## Traditional RAG vs Vectorless

<img src="https://raw.githubusercontent.com/vectorlessflow/vectorless/main/docs/design/comparison.svg" alt="Traditional RAG vs Vectorless">

| Aspect | Traditional RAG | Vectorless |
|--------|----------------|------------|
| **Infrastructure** | Vector DB + Embedding Model | Just LLM API |
| **Document Structure** | Lost in chunking | Preserved |
| **Context** | Fragment only | Section + surrounding context |
| **Setup Time** | Hours to Days | Minutes |
| **Best For** | Unstructured text | Structured documents |

## Example

**Input:**
```
Document: 100-page technical manual (PDF)
Query: "How do I reset the device?"
```

**Output:**
```
Answer: "To reset the device, hold the power button for 10 seconds 
until the LED flashes blue, then release..."

Source: Chapter 4 > Section 4.2 > Reset Procedure
```

## When to Use

✅ **Good fit:**
- Technical documentation
- Manuals and guides
- Structured reports
- Policy documents
- Any document with clear hierarchy

❌ **Not ideal:**
- Unstructured text (tweets, chat logs)
- Very short documents (< 1 page)
- Pure Q&A datasets without structure

## Quick Start

<details open>
<summary><b>Python</b></summary>

```bash
pip install vectorless
```

```python
from vectorless import Engine, IndexContext

# Create engine (uses OPENAI_API_KEY env var)
engine = Engine(workspace="./data")

# Index a document
ctx = IndexContext.from_file("./report.pdf")
doc_id = engine.index(ctx)

# Query
result = engine.query(doc_id, "What is the total revenue?")
print(f"Answer: {result.content}")
```

</details>

<details>
<summary><b>Rust</b></summary>

```toml
[dependencies]
vectorless = "0.1"
```

```bash
cp vectorless.example.toml ./vectorless.toml
```

```rust
use vectorless::Engine;

#[tokio::main]
async fn main() -> vectorless::Result<()> {
    let client = Engine::builder()
        .with_workspace("./workspace")
        .build()?;

    let doc_id = client.index("./document.pdf").await?;

    let result = client.query(&doc_id,
        "What are the system requirements?").await?;

    println!("Answer: {}", result.content);
    println!("Source: {}", result.path);

    Ok(())
}
```

</details>

## Features

| Feature | Description |
|---------|-------------|
| **Zero Infrastructure** | No vector DB, no embedding model — just an LLM API |
| **Multi-format Support** | PDF, Markdown, DOCX, HTML out of the box |
| **Incremental Updates** | Add/remove documents without full re-index |
| **Traceable Results** | See the exact navigation path taken |
| **Feedback Learning** | Improves from user feedback over time |
| **Multi-turn Queries** | Handles complex questions with decomposition |

## Configuration

### Zero Configuration (Recommended)

Just set `OPENAI_API_KEY` and you're ready to go:

```bash
export OPENAI_API_KEY="sk-..."
```

<details>
<summary><b>Python</b></summary>

```python
from vectorless import Engine

# Uses OPENAI_API_KEY from environment
engine = Engine(workspace="./data")
```

</details>

<details>
<summary><b>Rust</b></summary>

```rust
use vectorless::Engine;

let client = Engine::builder()
    .with_workspace("./workspace")
    .build().await?;
```

</details>

### Environment Variables

| Variable | Description |
|----------|-------------|
| `OPENAI_API_KEY` | LLM API key |
| `VECTORLESS_MODEL` | Default model (e.g., `gpt-4o-mini`) |
| `VECTORLESS_ENDPOINT` | API endpoint URL |
| `VECTORLESS_WORKSPACE` | Workspace directory |

### Advanced Configuration

For fine-grained control, use a config file:

```bash
cp config.toml ./vectorless.toml
```

<details>
<summary><b>Python</b></summary>

```python
from vectorless import Engine

# Use full configuration file
engine = Engine(config_path="./vectorless.toml")

# Or override specific settings
engine = Engine(
    config_path="./vectorless.toml",
    model="gpt-4o",  # Override model from config
)
```

</details>

<details>
<summary><b>Rust</b></summary>

```rust
use vectorless::Engine;

// Use full configuration file
let client = Engine::builder()
    .with_config_path("./vectorless.toml")
    .build().await?;

// Or override specific settings
let client = Engine::builder()
    .with_config_path("./vectorless.toml")
    .with_model("gpt-4o", None)  // Override model
    .build().await?;
```

</details>

### Configuration Priority

Later overrides earlier:

1. Default configuration
2. Auto-detected config file (`vectorless.toml`, `config.toml`, `.vectorless.toml`)
3. Explicit config file (`config_path` / `with_config_path`)
4. Environment variables
5. Constructor/builder parameters (highest priority)

## Architecture

<img src="https://raw.githubusercontent.com/vectorlessflow/vectorless/main/docs/design/architecture.svg" alt="Architecture">

### Core Components

- **Index Pipeline** — Parses documents, builds tree, generates summaries
- **Retrieval Pipeline** — Analyzes query, navigates tree, returns results
- **Pilot** — LLM-powered navigator that guides retrieval decisions
- **Metrics Hub** — Unified observability for LLM calls, retrieval, and feedback

## Examples

See the [examples/](examples/) directory for more usage patterns.

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
