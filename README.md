<div align="center">

<img src="https://vectorless.dev/img/with-title.png" alt="Vectorless" width="400">

<h1>Reasoning-based Document Engine</h1>
<h5>Reason, don't vector · Structure, not chunks · Agents, not embeddings</h5>

[![PyPI](https://img.shields.io/pypi/v/vectorless.svg)](https://pypi.org/project/vectorless/)
[![PyPI Downloads](https://static.pepy.tech/badge/vectorless/month)](https://pepy.tech/projects/vectorless)
[![Crates.io](https://img.shields.io/crates/v/vectorless.svg)](https://crates.io/crates/vectorless)
[![Crates.io Downloads](https://img.shields.io/crates/d/vectorless.svg)](https://crates.io/crates/vectorless)
[![Docs](https://docs.rs/vectorless/badge.svg)](https://docs.rs/vectorless)
[![License](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE)

</div>

**Vectorless** is a reasoning-native document engine written in Rust. It compiles documents into navigable trees, then dispatches **multiple agents** to find exactly what's relevant across your **PDFs, Markdown, reports, contracts**. No embeddings, no chunking, no approximate nearest neighbors. Every retrieval is a **reasoning** act.

Light up a star and shine with us! ⭐

## Three Rules
- **Reason, don't vector.** Retrieval is a reasoning act, not a similarity computation.
- **Model fails, we fail.** No heuristic fallbacks, no silent degradation.
- **No thought, no answer.** Only reasoned output counts as an answer.

## Why Vectorless

Traditional RAG systems split documents into chunks, embed them into vectors, and retrieve by similarity. Vectorless takes a different approach: it preserves document structure as a navigable tree and lets agents reason through it.

| | Embedding-Based RAG | Vectorless |
|---|---|---|
| **Indexing** | Chunk → embed → vector store | Parse → compile → document tree |
| **Retrieval** | Cosine similarity (approximate) | Multi-agent navigation (exact) |
| **Structure** | Destroyed by chunking | Preserved as first-class tree |
| **Query handling** | Keyword/similarity match | Intent classification + decomposition |
| **Multi-hop reasoning** | Not supported | Orchestrator replans dynamically |
| **Output** | Retrieved chunks | Original text passages, exact |
| **Failure mode** | Silent degradation | Explicit — no reasoning, no answer |

## How It Works

### Four-Artifact Index Architecture

When a document is indexed, the compile pipeline builds four artifacts:

```
Content Layer          Navigation Layer              Reasoning Index            Document Card
DocumentTree          NavigationIndex               ReasoningIndex            DocCard
(TreeNode)            (NavEntry, ChildRoute)        (topic_paths, hot_nodes)  (title, overview,
      │                      │                              │                 question hints)
      │                      │                              │                    │
 Agent reads           Agent reads every            Agent's targeted        Orchestrator reads
 only on cat           decision round               search tool (grep)      for multi-doc routing
```

- **Content Layer** — The raw document tree. The agent only accesses this when reading specific paragraphs (`cat`).
- **Navigation Layer** — Each non-leaf node stores an overview, question hints, and child routes (title + description). The agent reads this every round to decide where to go next.
- **Reasoning Index** — Keyword-topic mappings with weights. Provides the agent's `grep` tool with structured keyword data for targeted search within a document.
- **DocCard** — A compact document-level summary. The Orchestrator reads DocCards to decide which documents to navigate in multi-document queries, without loading full documents.

This separation means the agent makes routing decisions from lightweight metadata, not by scanning full content.

### Agent-Based Retrieval

```
Engine.query("What drove the revenue decline?")
  │
  ├─ Query Understanding ── intent, concepts, strategy (LLM)
  │
  ├─ Orchestrator ── analyzes query, dispatches Workers
  │   │
  │   ├─ Worker 1 ── ls → cd "Financials" → ls → cd "Revenue" → cat
  │   └─ Worker 2 ── ls → cd "Risk Factors" → grep "decline" → cat
  │   │
  │   └─ evaluate ── insufficient? → replan → dispatch new paths → loop
  │
  └─ Fusion ── dedup, LLM-scored relevance, return with source attribution
```

Worker navigation commands:

| Command | Action | Reads |
|---------|--------|-------|
| `ls` | List child sections | Navigation Layer (ChildRoute) |
| `cd` | Enter a child section | Navigation Layer |
| `cat` | Read content at current node | Content Layer (DocumentTree) |
| `grep` | Search by keyword | Reasoning Index (topic_paths) |

The Orchestrator evaluates Worker results after each round. If evidence is insufficient, it **replans** — adjusting strategy, dispatching new paths, or deepening exploration. This continues until enough evidence is collected.

## Quick Start

```bash
pip install vectorless
```

```python
import asyncio
from vectorless import Engine, IndexContext, QueryContext

async def main():
    engine = Engine(api_key="sk-...", model="gpt-4o", endpoint="https://api.openai.com/v1")

    # Index a document
    result = await engine.index(IndexContext.from_path("./report.pdf"))
    doc_id = result.doc_id

    # Query
    result = await engine.query(
        QueryContext("What is the total revenue?").with_doc_ids([doc_id])
    )
    print(result.single().content)

asyncio.run(main())
```

## Key Features

- **Rust Core** — The entire engine (indexing, retrieval, agent, storage) is implemented in Rust for performance and reliability. Python SDK via PyO3 bindings and a CLI are also provided.
- **Multi-Agent Retrieval** — Every query is handled by multiple cooperating agents: an Orchestrator plans and evaluates, Workers navigate documents. Each retrieval is a reasoning act — not a similarity score, but a sequence of LLM decisions about where to look, what to read, and when to stop.
- **Zero Vectors** — No embedding model, no vector store, no similarity search. This eliminates a class of failure modes: wrong chunk boundaries, stale embeddings, and similarity-score false positives.
- **Tree Navigation** — Documents are compiled into hierarchical trees that preserve the original structure — headings, sections, paragraphs, lists. Workers navigate this tree the way a human would: scan the table of contents, jump to the relevant section, read the passage.
- **Document-Exact Output** — Returns original text passages from the source document. No synthesis, no rewriting, no hallucinated content. What you get is what was written.
- **Multi-Document Orchestration** — Query across multiple documents with a single call. The Orchestrator dispatches Workers, evaluates evidence, and fuses results. When one document is insufficient, it replans and expands the search scope.
- **Query Understanding** — Every query passes through LLM-based intent classification, concept extraction, and strategy selection. Complex queries are decomposed into sub-queries. The system adapts its navigation strategy based on whether the query is factual, analytical, comparative, or navigational.
- **Checkpointable Pipeline** — The 8-stage compile pipeline writes checkpoints at each stage. If indexing is interrupted (LLM rate limit, network failure), it resumes from the last completed stage — no wasted work.
- **Incremental Updates** — Content fingerprinting detects changes at the node level. Re-indexing a modified document only recompiles the changed sections and their dependents.

## Supported Documents

- **PDF** — Full text extraction with page metadata
- **Markdown** — Structure-aware parsing (headings, lists, code blocks)

## Resources

- [Documentation](https://vectorless.dev) — Guides, architecture, API reference
- [Rust API Docs](https://docs.rs/vectorless) — Auto-generated crate documentation
- [PyPI](https://pypi.org/project/vectorless/) — Python package
- [Crates.io](https://crates.io/crates/vectorless) — Rust crate
- [Examples](examples/) — Complete usage patterns for Python and Rust

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
