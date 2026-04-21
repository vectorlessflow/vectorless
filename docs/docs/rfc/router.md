---
sidebar_position: 1
---

# RFC: Document Router Layer for Large-Scale Retrieval

## Summary

Vectorless currently sends all workspace documents to the Orchestrator's analysis phase, which uses a single LLM call to read every DocCard and select relevant ones. This works well for small workspaces (1–100 documents) but breaks down at scale — 1000+ DocCards exceed token budgets, increase latency, and make LLM selection unreliable.

This RFC proposes a **Router layer** that sits between `Engine.query()` and the Orchestrator, using compile-stage artifacts to pre-filter documents before dispatching Workers.

## Problem

```
Current flow (workspace scope):

Engine.query(workspace)
  → resolve_scope() → all doc_ids (potentially 1000+)
  → load_documents() → load ALL (tree + nav_index + reasoning_index)
  → Orchestrator.analyze()
    → LLM reads ALL DocCards in one prompt  ← bottleneck
    → selects docs → dispatch Workers
```

Issues at 1000+ documents:

- **Token budget**: 1000 DocCards × ~200 tokens = ~200K tokens — exceeds context windows
- **Cost**: One massive LLM call just for document selection
- **Latency**: Loading all document trees upfront is wasteful when only a fraction will be used
- **Quality**: LLM selection degrades when presented with too many options

## Proposed Solution

Insert a **Router** that uses compile-stage artifacts (DocCard, ReasoningIndex, DocumentGraph) for coarse filtering, narrowing candidates to a manageable set before the Orchestrator's LLM-based analysis.

```
Proposed flow:

Engine.query(workspace)
  → resolve_scope() → all doc_ids
  → Router::route(query, doc_ids) → top-K candidates (10–20)
  → load_documents(top_K) → only K documents
  → Orchestrator.run() (reduced candidate set)
    → analyze: LLM reads K DocCards → precise selection
    → dispatch Workers → navigate trees
```

The Router does **not** replace the Orchestrator's analysis. It narrows the input so the LLM can make a better-informed, cheaper decision.

## Data Sources

The Router leverages artifacts already produced by the compile pipeline — **no additional LLM calls at index time**.

| Artifact | Fields Used by Router | Produced By |
|----------|----------------------|-------------|
| **DocCard** | `title`, `overview`, `topic_tags`, `question_hints`, `sections` | NavigationIndexStage |
| **ReasoningIndex** | `topic_paths` (keyword → node mappings) | ReasoningIndexStage |
| **DocumentGraph** | cross-document edges, shared keywords | DocumentGraphBuilder |

These are lightweight (no tree content), fast to load, and already persisted in the workspace.

## Scoring Strategy

The Router combines three scoring signals:

### 1. BM25 on DocCard text (lexical match)

Build a BM25 index over each document's DocCard searchable text:

```
searchable_text = f"{title} {overview} {question_hints} {topic_tags} {section_descriptions}"
```

The existing `Bm25Engine<K>` in `rust/src/scoring/bm25.rs` supports per-field weighting. For Router use, we weight title and topic_tags higher than section descriptions.

### 2. Keyword overlap (concept match)

Use `QueryPlan.key_concepts` (from query understanding) to match against each document's:

- `DocCard.topic_tags`
- `ReasoningIndex.topic_paths` keys

Score = Jaccard similarity between query concepts and document keywords.

### 3. Graph-based boost (contextual relevance)

If the DocumentGraph is available, documents connected to high-scoring candidates receive a boost. This captures the intuition that related documents may be co-relevant.

### Fusion

```
final_score = w_bm25 * normalize(bm25_score)
            + w_keyword * keyword_overlap
            + w_graph * graph_boost
```

Default weights: `w_bm25 = 0.5`, `w_keyword = 0.3`, `w_graph = 0.2`

### LLM-assisted routing (optional)

When BM25 + keyword scores are ambiguous (e.g., top candidates have similar scores), the Router can optionally invoke the LLM to rank the top-M candidates. This is a lightweight call — the LLM only sees K DocCard summaries (not full trees), making it orders of magnitude cheaper than the current all-DocCards approach.

```
RouteMode::Fast     → BM25 + keyword + graph (no LLM)
RouteMode::Balanced → BM25 + keyword + graph, then LLM top-M if ambiguous
RouteMode::Precise  → BM25 + keyword + graph + LLM top-M always
```

## Activation Threshold

The Router is only activated when the workspace exceeds a configurable document count:

```rust
RouterConfig {
    activate_threshold: 20,   // only route when docs > 20
    max_candidates: 15,       // top-K to pass to Orchestrator
    bm25_top_k: 50,           // BM25 initial retrieval size
    mode: RouteMode::Fast,    // default: no LLM in Router
}
```

Below the threshold, the current flow (all DocCards → Orchestrator.analyze) is used unchanged.

## Incremental Index Maintenance

The Router's BM25 index is updated incrementally alongside document indexing:

- **Document indexed**: Extract DocCard → `router.upsert(doc_id, card, keywords)`
- **Document removed**: `router.remove(doc_id)`
- **Graph rebuilt**: `router.update_graph(new_graph)`

No full re-index needed — the Router stays in sync with the workspace.

## Lazy Document Loading (future optimization)

Currently `load_documents()` loads full `DocumentTree + NavigationIndex + ReasoningIndex` for all candidates. With the Router, we can defer tree loading until Worker dispatch:

1. Router + Orchestrator.analyze: only need DocCards (lightweight)
2. Orchestrator.dispatch: load DocumentTree per-Worker, on demand

This reduces memory pressure when the Router selects 15 candidates from 1000 documents but the Orchestrator only dispatches 5 Workers.

## Module Structure

```
rust/src/router/
├── mod.rs           # DocumentRouter, RouteResult, ScoredCandidate
├── scorer.rs        # BM25 + keyword + graph fusion scoring
└── config.rs        # RouterConfig, RouteMode
```

Integration points:

- `rust/src/client/engine.rs` — insert `Router::route()` in `query()`
- `rust/src/config/mod.rs` — add `router: RouterConfig`
- `python/src/lib.rs` — expose RouterConfig to Python SDK

## What This Is Not

- Not a replacement for the Orchestrator — Router is coarse filter, Orchestrator is precise selector
- Not an embedding layer — uses BM25 + keywords + graph, no vector similarity required
- Not a "vector backtrack" — this is a pragmatic engineering layer that happens to use lexical matching

## Open Questions

1. **Score calibration**: How to normalize BM25 scores across workspaces with different corpus sizes? Min-max normalization may not work well with very few documents. Consider quantile-based normalization.

2. **Cold start**: New documents have no graph edges and no hot-node history. Should new docs get a freshness boost?

3. **Multi-hop routing**: Should the Router consider re-routing after the first Orchestrator iteration finds nothing? Or is one-shot routing sufficient given the supervisor loop can replan?

4. **Thread safety**: The Router holds a mutable BM25 index. Need to decide between `RwLock<DocumentRouter>` or rebuild-on-query from workspace data.
