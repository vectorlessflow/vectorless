<div align="center">

<img src="https://vectorless.dev/img/with-title.png" alt="Vectorless" width="400">

<h1>Document Understanding Engine for AI</h1>
<h3>Reason, don't vector · Structure, not chunks · Think, then answer</h3>

[![PyPI](https://img.shields.io/pypi/v/vectorless.svg)](https://pypi.org/project/vectorless/)
[![PyPI Downloads](https://static.pepy.tech/badge/vectorless/month)](https://pepy.tech/projects/vectorless)
[![Docs](https://docs.rs/vectorless/badge.svg)](https://docs.rs/vectorless)
[![License](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE)

</div>

**Vectorless** is a document understanding engine for AI. It compiles documents into structured trees of meaning, then dispatches multiple agents to reason through headings, sections, and paragraphs — evaluating how each part relates to the whole. The problem it solves is not "where to look", but "what does this mean in context". Every answer is a reasoning act, not a retrieval result.

Light up a star and shine with us! ⭐

## Three Rules
- **Reason, don't vector.** Understanding is reasoning, not similarity.
- **Model fails, we fail.** No heuristic fallbacks, no silent degradation.
- **No thought, no answer.** Only reasoned output counts as an answer.

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

### Agent-Based Understanding

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
  └─ Synthesis ── dedup, evidence scoring, reasoned answer with source chain
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
