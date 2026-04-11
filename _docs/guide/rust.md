# Rust Usage Guide

## Installation

```toml
[dependencies]
vectorless = "0.1"
tokio = { version = "1", features = ["full"] }
```

## Configuration

### Engine Builder

```rust
use vectorless::client::{EngineBuilder, IndexContext};

// Zero configuration — uses OPENAI_API_KEY env var
let engine = EngineBuilder::new()
    .with_workspace("./data")
    .build()
    .await?;
```

### Custom Model and API Key

```rust
let engine = EngineBuilder::new()
    .with_workspace("./data")
    .with_model("gpt-4o-mini", Some("sk-your-key"))
    .build()
    .await?;
```

### OpenAI Preset

```rust
let engine = EngineBuilder::new()
    .with_workspace("./data")
    .with_openai("sk-your-key")
    .build()
    .await?;
```

### Custom Endpoint

```rust
// DeepSeek, Azure OpenAI, local LLM, etc.
let engine = EngineBuilder::new()
    .with_workspace("./data")
    .with_model("deepseek-chat", Some("sk-key"))
    .with_endpoint("https://api.deepseek.com/v1")
    .build()
    .await?;
```

### Configuration File

```rust
// Auto-detects vectorless.toml, config.toml, .vectorless.toml
let engine = EngineBuilder::new().build().await?;

// Explicit config file
let engine = EngineBuilder::new()
    .with_config_path("./vectorless.toml")
    .build()
    .await?;
```

### Preset Modes

```rust
// Fast mode — keyword-based, fewer iterations
let engine = EngineBuilder::new()
    .with_workspace("./data")
    .fast()
    .build()
    .await?;

// Precise mode — MCTS-based, more iterations
let engine = EngineBuilder::new()
    .with_workspace("./data")
    .precise()
    .build()
    .await?;
```

**Configuration priority** (highest → lowest):
1. Builder methods
2. Environment variables
3. Explicit config file
4. Auto-detected config file
5. Default configuration

## Indexing

### From File Path

```rust
let doc_id = engine.index(IndexContext::from_path("./report.pdf")).await?;
let doc_id = engine.index(IndexContext::from_path("./readme.md")).await?;
let doc_id = engine.index(IndexContext::from_path("./doc.docx")).await?;
```

Format is auto-detected from file extension.

### From String Content

```rust
use vectorless::parser::DocumentFormat;

let ctx = IndexContext::from_content("# Title\nContent", DocumentFormat::Markdown)
    .with_name("manual");
let doc_id = engine.index(ctx).await?;
```

### From Bytes

```rust
let pdf_bytes = std::fs::read("./report.pdf")?;
let ctx = IndexContext::from_bytes(pdf_bytes, DocumentFormat::Pdf);
let doc_id = engine.index(ctx).await?;
```

### Index Options

```rust
use vectorless::client::IndexOptions;
use vectorless::client::IndexMode;

let options = IndexOptions {
    mode: IndexMode::Force,          // Always re-index
    generate_summaries: true,
    include_text: true,
    generate_ids: true,
    generate_description: false,
};

let ctx = IndexContext::from_path("./doc.md")
    .with_name("My Document")
    .with_options(options);
```

**IndexMode types:**
- `Default` — Skip if already indexed
- `Force` — Always re-index
- `Incremental` — Only re-index changed files

## Querying

### Basic Query

```rust
let result = engine.query(&doc_id, "What is the total revenue?").await?;

println!("Content: {}", result.content);
println!("Score: {}", result.score);
println!("Nodes: {:?}", result.node_ids);
println!("Is empty: {}", result.is_empty());
```

### With Retrieve Options

```rust
use vectorless::retrieval::RetrieveOptions;
use vectorless::retrieval::StrategyPreference;

let options = RetrieveOptions::new()
    .with_top_k(5)
    .with_beam_width(3)
    .with_max_iterations(10)
    .with_min_score(0.1)
    .with_strategy(StrategyPreference::Auto)
    .with_sufficiency_check(true)
    .with_max_tokens(4000)
    .with_streaming(false);

let result = engine.query_with_options(&doc_id, "question", &options).await?;
```

### Strategy Selection

```rust
use vectorless::retrieval::StrategyPreference;

StrategyPreference::Auto              // Automatic based on query complexity
StrategyPreference::ForceKeyword      // Fast, no LLM calls
StrategyPreference::ForceSemantic     // Embedding-based
StrategyPreference::ForceLlm          // Deep LLM reasoning
StrategyPreference::ForceHybrid       // BM25 + LLM refinement
StrategyPreference::ForceCrossDocument // Multi-document search
StrategyPreference::ForcePageRange    // PDF page range filtering
```

## Streaming

```rust
use vectorless::retrieval::{RetrieveOptions, RetrieveEvent};

let options = RetrieveOptions::new().with_streaming(true);
let mut rx = engine.query_stream(&doc_id, "architecture", &options).await?;

while let Some(event) = rx.recv().await {
    match event {
        RetrieveEvent::Started { query, strategy } => {
            println!("Started: {} ({})", query, strategy);
        }
        RetrieveEvent::StageCompleted { stage, elapsed_ms } => {
            println!("Stage {} done in {}ms", stage, elapsed_ms);
        }
        RetrieveEvent::NodeVisited { node_id, title, score } => {
            println!("Visited: {} (score: {:.2})", title, score);
        }
        RetrieveEvent::ContentFound { node_id, title, preview, score } => {
            println!("Found: {} — {} ({:.2})", title, preview, score);
        }
        RetrieveEvent::Backtracking { from, to, reason } => {
            println!("Backtrack: {} → {} ({})", from, to, reason);
        }
        RetrieveEvent::SufficiencyCheck { level, tokens } => {
            println!("Sufficiency: {:?}, {} tokens", level, tokens);
        }
        RetrieveEvent::Completed { response } => {
            println!("Done: {}", response.content);
            break;
        }
        RetrieveEvent::Error { message } => {
            eprintln!("Error: {}", message);
            break;
        }
    }
}
```

## Document Management

```rust
// List documents
let docs = engine.list_documents().await?;
for doc in &docs {
    println!("{}: {} ({})", doc.id, doc.name, doc.format);
}

// Check existence
if engine.exists(&doc_id).await? { /* ... */ }

// Remove
engine.remove(&doc_id).await?;

// Batch remove
let ids = &["doc1", "doc2"];
engine.batch_remove(ids).await?;

// Clear all
let count = engine.clear().await?;
```

## Document Graph

### Building a Graph

```rust
use vectorless::document::{DocumentGraph, DocumentGraphConfig, DocumentGraphNode, WeightedKeyword};
use vectorless::index::graph_builder::DocumentGraphBuilder;

let config = DocumentGraphConfig {
    enabled: true,
    min_keyword_jaccard: 0.05,
    min_shared_keywords: 2,
    max_keywords_per_doc: 50,
    max_edges_per_node: 20,
    retrieval_boost_factor: 0.15,
};

let mut builder = DocumentGraphBuilder::new(config);

builder.add_document("doc1", "Rust Guide", "md", 35, keywords(&[
    ("ownership", 0.95), ("borrowing", 0.90), ("lifetimes", 0.85),
]));
builder.add_document("doc2", "Async Rust", "md", 28, keywords(&[
    ("async", 0.95), ("lifetimes", 0.60), ("tokio", 0.90),
]));

let graph = builder.build();
```

### Exploring the Graph

```rust
// Get neighbors (connected documents)
let neighbors = graph.get_neighbors("doc1");
for edge in neighbors {
    println!("→ {} [weight={:.3}, jaccard={:.3}]",
        edge.target_doc_id, edge.weight, edge.evidence.keyword_jaccard);
}

// Find documents by keyword
let entries = graph.find_by_keyword("lifetimes");
for e in entries {
    println!("{} (weight: {:.2})", e.doc_id, e.weight);
}

// Stats
println!("Documents: {}", graph.node_count());
println!("Edges: {}", graph.edge_count());
```

## Custom Pilot

```rust
use vectorless::retrieval::pilot::{Pilot, PilotDecision, SearchState, PilotConfig};

struct MyPilot;

#[async_trait]
impl Pilot for MyPilot {
    async fn guide(&self, state: &SearchState) -> PilotDecision {
        // Custom navigation logic
        PilotDecision::Continue
    }
}
```

## Environment Variables

| Variable | Description |
|----------|-------------|
| `OPENAI_API_KEY` | LLM API key |
| `VECTORLESS_MODEL` | Default model |
| `VECTORLESS_ENDPOINT` | Custom API endpoint URL |
| `VECTORLESS_WORKSPACE` | Workspace directory |

## Examples

See [examples/rust/](../../examples/rust/) for complete examples:

| Example | Description |
|---------|-------------|
| `basic` | Core API: index, list, query |
| `advanced` | Full configuration file usage |
| `streaming` | Streaming retrieval with events |
| `document_graph` | Cross-document concept graph |
| `session` | Multi-document session operations |
| `feedback_learning` | User feedback and Pilot adaptation |
| `custom_pilot` | Custom navigation guidance |
| `reference_following` | In-document reference resolution |
| `strategy_hybrid` | BM25 + LLM hybrid retrieval |
| `strategy_cross_document` | Multi-document search |
| `strategy_page_range` | PDF page range filtering |
| `multi_format` | PDF, MD, DOCX, HTML handling |
| `events` | Event system monitoring |
| `storage_backend` | Custom storage implementations |
| `storage_compression` | Compression support |
| `storage_workspace` | Workspace and caching |
| `content_aggregation` | Content scoring and relevance |
| `batch_processing` | Batch document processing |
| `cli_tool` | Building CLI tools |
