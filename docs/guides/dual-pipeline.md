# Understanding the Dual Pipeline

Vectorless uses a **dual pipeline architecture** that separates document processing from retrieval. This design enables efficient indexing and intelligent retrieval.

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                           Vectorless Architecture                            │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│   ┌─────────────────────────────┐     ┌─────────────────────────────┐      │
│   │      INDEX PIPELINE         │     │    RETRIEVAL PIPELINE       │      │
│   │                             │     │                             │      │
│   │  Parse → Build → Enrich    │     │  Analyze → Plan → Search    │      │
│   │    ↓       ↓       ↓       │     │     ↓        ↓       ↓      │      │
│   │  Enhance → Optimize →      │     │  Evaluate (Sufficiency)     │      │
│   │    Persist                  │     │     ↑_____________│         │      │
│   │                             │     │     │ (NeedMoreData)│         │      │
│   └─────────────────────────────┘     └─────────────────────────────┘      │
│                 │                                    ▲                      │
│                 └──────────── Workspace ─────────────┘                       │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

## Index Pipeline

The Index Pipeline processes documents and builds a searchable tree structure.

### Stages

| Stage | Purpose |
|-------|---------|
| **Parse** | Extract content from file (MD, PDF, DOCX, HTML) |
| **Build** | Construct hierarchical document tree |
| **Enrich** | Add metadata, TOC, references |
| **Enhance** | Generate summaries (optional) |
| **Optimize** | Prune, compress, optimize tree |
| **Persist** | Save to workspace storage |

### Example

```rust
// Index pipeline is triggered automatically
let doc_id = engine.index(IndexContext::from_path("./manual.md")).await?;

// With summary generation
let doc_id = engine.index(
    IndexContext::from_path("./manual.md")
        .with_options(IndexOptions::new().with_summaries())
).await?;
```

## Retrieval Pipeline

The Retrieval Pipeline processes queries and retrieves relevant content.

### Stages

| Stage | Purpose |
|-------|---------|
| **Analyze** | Analyze query complexity, extract keywords |
| **Plan** | Select retrieval strategy and algorithm |
| **Search** | Navigate tree to find candidates |
| **Evaluate** | Check sufficiency, aggregate content |

### The Evaluate Stage

The Evaluate stage is crucial - it determines if retrieved content is sufficient:

```text
                    ┌─────────────┐
                    │   Search    │
                    └──────┬──────┘
                           │
                           ▼
                    ┌─────────────┐
                    │  Evaluate   │
                    └──────┬──────┘
                           │
              ┌────────────┼────────────┐
              │            │            │
              ▼            ▼            ▼
        Sufficient    PartialSufficient  Insufficient
              │            │            │
              ▼            ▼            ▼
           Return      More Search    Expand Beam
                       (1 iteration)  (2 iterations)
```

### Retrieval Strategies

```rust
// Three built-in strategies:

// 1. Keyword - Fast, exact matching
// 2. LLM - Semantic understanding via Pilot
// 3. Structure - Hierarchy-aware navigation
```

## The Pilot System

Pilot is the "brain" of the Retrieval Pipeline:

- **Query Analysis**: Understands what the user is asking
- **Context Building**: Creates navigation context from TOC
- **Decision Making**: Decides which branches to explore
- **Fallback**: Algorithm takes over when LLM fails

See [The Pilot System](./pilot-system.md) for details.

## Data Flow

```
Document ──► Index Pipeline ──► Workspace
                                       │
Query ──► Retrieval Pipeline ──────────┘
                    │
                    ▼
              RetrievalResult
              ├── content
              ├── node_ids
              ├── confidence
              └── trace
```

## Session-Based Operations

For multi-document operations, use sessions:

```rust
// Create a session
let session = engine.session().await;

// Index multiple documents
session.index(IndexContext::from_path("./doc1.md")).await?;
session.index(IndexContext::from_path("./doc2.md")).await?;

// Query across all documents
let results = session.query_all("What is the architecture?").await?;

for result in results {
    println!("From {}: {}", result.doc_id, result.content);
}
```

## See Also

- [Multi-Strategy Retrieval](./multi-strategy.md)
- [Content Aggregation](./content-aggregation.md)
- [Sufficiency Checking](./sufficiency.md)
