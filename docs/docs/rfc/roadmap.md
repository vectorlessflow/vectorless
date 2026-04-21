---
sidebar_position: 2
---

# Roadmap: The Ultimate Document Retrieval Tool

> Focus: structured document retrieval — precise, reliable, indispensable.
> The "jq of document retrieval".

## Scope

Focus on the document retrieval vertical — no code retrieval, no general knowledge platform. Build a complete Python developer experience layer on top of the Rust core engine, with broader format support and finer-grained parsing.

## Phase Overview

| Phase | Focus | Language |
|-------|-------|----------|
| A1 | Router Layer — support 1000+ document workspaces | Rust |
| A2 | Document Formats — HTML, DOCX, LaTeX | Rust |
| A3 | Parsing Precision — tables, figures, footnotes | Rust |
| A4 | Python Ecosystem — CLI, Pythonic API, framework integration | Python |
| A5 | Domain Optimization — legal, financial, technical documents | Rust |
| A6 | Performance & Reliability — lazy loading, caching, concurrency | Rust |

Dependencies:

```
A1 (Router) ────→ A6 (Lazy Loading) ────→ A2 (Formats)
                                            ↓
                                       A3 (Precision)
                                            ↓
A4 (Python, can run in parallel)
                                            ↓
                                       A5 (Domain)
```

---

## A1: Router Layer

**Goal:** Support retrieval across 1000+ document workspaces.

Full design: [RFC: Document Router](./router.md)

Key ideas:

- Insert a Router between `Engine.query()` and the Orchestrator
- Use compile-stage artifacts (DocCard + ReasoningIndex + DocumentGraph) for coarse filtering
- BM25 + keyword overlap + graph boost — three-signal scoring fusion
- Optional LLM-assisted routing (LLM ranks top-M candidates when scores are ambiguous)
- Only activates when document count exceeds a configurable threshold

Module structure:

```
rust/src/router/
├── mod.rs           # DocumentRouter, RouteResult, ScoredCandidate
├── scorer.rs        # BM25 + keyword + graph fusion scoring
└── config.rs        # RouterConfig, RouteMode
```

Estimated: ~600 lines Rust, no new dependencies.

---

## A2: Document Format Support

**Goal:** Support HTML, DOCX, LaTeX in addition to PDF and Markdown.

### HTML Parsing

```
HTML DOM → hierarchical tree structure
  <h1>–<h6> → depth-mapped nodes
  <p>, <li>, <td> → content nodes
  <table> → special handling (text + structure)
  <code>, <pre> → preserve formatting
```

Challenge: HTML documents often have deep nesting (`div > div > div`) that doesn't represent semantic structure. Need heuristics to skip decorative containers.

### DOCX Parsing

```
DOCX = ZIP archive
  word/document.xml → paragraph extraction
  <w:pStyle w:val="Heading1"/> → heading level
  <w:p> → paragraph content
  Style inheritance → heading/body classification
```

### LaTeX Parsing

```
Regex-based extraction:
  \section{...} → depth-0 node
  \subsection{...} → depth-1 node
  \begin{...} environments → content blocks
```

### Tasks

| # | Task | File |
|---|------|------|
| 1 | HTML parser | `rust/src/index/parse/html.rs` |
| 2 | DOCX parser | `rust/src/index/parse/docx.rs` |
| 3 | LaTeX parser | `rust/src/index/parse/latex.rs` |
| 4 | Format detection | extend `detect_format_from_path()` |
| 5 | IndexMode extension | `rust/src/index/pipeline.rs` |

New dependencies: `scraper = "0.22"`, `zip = "2"`

Estimated: ~800 lines Rust.

---

## A3: Parsing Precision

**Goal:** Fine-grained extraction of tables, figures, and footnotes.

### Current Limitations

`pdf-extract` produces flat text. Tables lose structure, figures are invisible, footnotes mix into body text.

### Table Extraction (PDF)

Use `lopdf` low-level access to detect text blocks with (x, y) coordinates, group by row and column, output as Markdown table strings. Insert as dedicated TreeNodes with `{type: "table"}` metadata.

### Figure Description (PDF)

Extract image streams via `lopdf`, send to LLM (vision-capable model), insert description as TreeNode with `{type: "figure"}` metadata. The only new LLM call in indexing — justified because figures often contain critical information invisible to text extraction.

### Cross-Reference Resolution

Resolve "see Section 3.2", "refer to Figure 4", "as noted in Table 2" to target TreeNodes. Enhances NavigationIndex with cross-reference edges for Worker navigation.

### Tasks

| # | Task | File |
|---|------|------|
| 1 | PDF table extraction | `rust/src/index/parse/pdf_table.rs` |
| 2 | PDF figure description | `rust/src/index/parse/pdf_figure.rs` |
| 3 | PDF footnote handling | `rust/src/index/parse/pdf_footnote.rs` |
| 4 | Markdown table parsing | `rust/src/index/parse/md_table.rs` |
| 5 | Cross-reference resolution | extend `rust/src/document/reference.rs` |

New dependency: `image = "0.25"`

Estimated: ~1000 lines Rust.

---

## A4: Python Ecosystem

**Goal:** Complete Python developer experience.

See the [Python ecosystem expansion plan](https://github.com/vectorlessflow/vectorless/blob/main/.claude/plans/shimmying-tumbling-hare.md) for full details.

| Phase | Content | Deliverable |
|-------|---------|-------------|
| 1 | CLI | `vectorless init/add/query/list/remove/ask/tree/stats/config` |
| 2 | Pythonic API | `errors.py`, `_engine.py`, `_query.py`, type stubs |
| 3 | High-level abstractions | `BatchIndexer`, `DocumentWatcher` |
| 4 | Framework integration | LangChain `BaseRetriever`, LlamaIndex adapter |
| 5 | Testing | Unit → Mock → E2E |

A4 runs in parallel with A1–A3 — the Python layer doesn't depend on new Rust features.

---

## A5: Domain Optimization

**Goal:** Domain-specific optimizations for legal, financial, and technical documents.

### Domain Template System

```rust
pub trait DomainTemplate: Send + Sync {
    fn name(&self) -> &str;
    fn detect(&self, tree: &DocumentTree, card: &DocCard) -> bool;
    fn enhance(&self, tree: &mut DocumentTree, card: &mut DocCard);
    fn domain_tags(&self, tree: &DocumentTree) -> Vec<String>;
}
```

| Domain | Optimizations |
|--------|--------------|
| **Legal** | Contract clause identification, article reference resolution, defined term tracking |
| **Financial** | KPI extraction from tables, reporting period detection, currency normalization |
| **Technical** | Code block extraction with language tags, API endpoint identification, version-aware sectioning |

Templates hook into the compile pipeline after the Enhance stage.

Estimated: ~500 lines Rust (framework + 2–3 built-in templates).

---

## A6: Performance & Reliability

**Goal:** Optimize memory, latency, and observability.

### Lazy Document Loading

Defer tree loading until Worker dispatch. Router + Orchestrator.analyze only need DocCards (lightweight). Each DocumentTree is 10–100x larger than its DocCard.

### Caching

- **Router cache**: Cache routing results keyed by `(query_hash, doc_ids_hash)`. Invalidate on document add/remove.
- **Query cache**: Same query + same documents = cached result. Useful for interactive mode.

### Subtree-Level Incremental Updates

Current incremental update detects file-level changes. Refine to diff affected subtrees and only re-compile changed portions. Can reduce re-indexing LLM calls by 50–80%.

### Metrics

| Metric | Source | Use Case |
|--------|--------|----------|
| Router latency | `router.route()` | Monitor routing overhead |
| Router cache hit rate | Router cache | Tune cache size |
| Lazy load count | Worker dispatch | Verify memory savings |

---

## Success Metrics

| Metric | Current | Target |
|--------|---------|--------|
| Max practical workspace size | ~100 docs | 10,000+ docs |
| Index time per doc (PDF, 50 pages) | ~30s | ~20s |
| Query latency (100 docs) | ~10s | ~8s |
| Query latency (1000 docs) | N/A | ~12s |
| Python install-to-query | Manual setup | < 5 minutes |
| Format support | PDF, Markdown | + HTML, DOCX, LaTeX |

---

## Execution Priority

```
Sprint 1: A1 (Router) + A4 Phase 1 (CLI)
Sprint 2: A6 (Lazy Loading) + A4 Phase 2 (Pythonic API)
Sprint 3: A2 (HTML, DOCX, LaTeX)
Sprint 4: A3 (Table, Figure, Footnote)
Sprint 5: A5 (Domain Templates) + A4 Phase 4 (Framework Integration)
```

A1 is the most critical enabler — without it, large-scale scenarios are not viable. A4 (Python) runs in parallel throughout.
