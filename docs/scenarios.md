# Vectorless Usage Scenarios

> Vectorless is an ultra-performant reasoning-native document intelligence engine for AI, with the core written in Rust. It transforms documents into rich semantic trees and uses LLMs to intelligently traverse the hierarchy — retrieving the most relevant content through structural reasoning and deep contextual understanding. **No vectors.**

## Engine Positioning

Vectorless is a **library/engine**, not a service. It provides:

- **Rust Library** — direct dependency via Cargo
- **Python SDK** — bindings via PyO3

HTTP servers, REST APIs, web frameworks — those are the user's responsibility, built on top of Vectorless.

---

## API Design

The engine exposes **exactly two methods**. All variations are expressed through context objects, keeping the interface minimal and stable.

### Core Interface

```rust
impl Engine {
    pub fn index(&self, ctx: IndexContext) -> Result<IndexResult>;
    pub fn query(&self, ctx: QueryContext) -> Result<QueryResult>;
}
```

### IndexContext

```rust
// Single file
engine.index(IndexContext::new("report.pdf"))?;

// Directory — recursive by default
engine.index(IndexContext::new("./docs/"))?;

// Multiple files
engine.index(IndexContext::new(vec!["a.pdf", "b.docx", "c.md"]))?;

// With options
engine.index(
    IndexContext::new("./legal_contracts/")
        .recursive(true)
        .workspace("legal")
        .summary_strategy(SummaryStrategy::Selective)
        .on_progress(|event| { /* progress callback */ })
)?;
```

**IndexContext fields:**

| Field | Type | Description |
|-------|------|-------------|
| `source` | `Source` | File path, directory, or list of files |
| `recursive` | `bool` | Recurse into subdirectories (default: `true`) |
| `workspace` | `Option<String>` | Target workspace name |
| `summary_strategy` | `SummaryStrategy` | Full / Lazy / Selective |
| `force` | `bool` | Re-index even if unchanged (default: `false`) |
| `formats` | `Option<Vec<Format>>` | Filter by format (default: all supported) |
| `on_progress` | `Option<Callback>` | Progress event callback |

### QueryContext

```rust
// Simple query — searches entire workspace
engine.query(QueryContext::new("认证模块的 token 刷新逻辑在哪？"))?;

// Scoped to specific documents
engine.query(
    QueryContext::new("违约责任条款有哪些变化？")
        .scope(vec!["contract_v1.docx", "contract_v2.docx"])
)?;

// With budget control
engine.query(
    QueryContext::new("Transformer 改进方向")
        .max_tokens(4000)
        .strategy(Strategy::Mcts)
)?;
```

**QueryContext fields:**

| Field | Type | Description |
|-------|------|-------------|
| `query` | `String` | The query text |
| `scope` | `Option<Scope>` | Restrict to specific documents / workspace |
| `max_tokens` | `Option<usize>` | Token budget for result content |
| `strategy` | `Option<Strategy>` | BeamSearch / MCTS / Hybrid (default: auto) |
| `include_reasoning` | `bool` | Return reasoning chain (default: `true`) |
| `depth_limit` | `Option<usize>` | Max tree traversal depth |

### QueryResult

```rust
pub struct QueryResult {
    pub content: String,                  // Retrieved content
    pub entries: Vec<ResultEntry>,        // Individual matches
    pub reasoning_chain: ReasoningChain,  // Why these results
    pub token_usage: TokenUsage,          // LLM tokens consumed
    pub strategy_used: Strategy,          // Which strategy was selected
}

pub struct ResultEntry {
    pub document: String,       // Source document name
    pub path: String,           // Tree path, e.g. "3.2.1"
    pub title: String,          // Section title
    pub content: String,        // Matched content
    pub confidence: f64,        // Confidence score
}

pub struct ReasoningChain {
    pub steps: Vec<ReasoningStep>,  // Ordered reasoning steps
}
```

### Python SDK

```python
import vectorless

engine = vectorless.Engine(
    workspace="./my_index",
    summary_model="gpt-4o-mini",
    retrieval_model="gpt-4o"
)

# Same two-method interface
engine.index(vectorless.IndexContext("./docs/"))
result = engine.query(vectorless.QueryContext("查询内容", max_tokens=4000))

print(result.content)
print(result.reasoning_chain)
```

### Design Principles

1. **Two methods, nothing else.** `index()` and `query()` are the entire public API.
2. **Context objects carry all variance.** New features = new fields on context, not new methods.
3. **Builder pattern for context.** `IndexContext::new(...).recursive(true).workspace("...")` follows Rust convention.
4. **Defaults are sensible.** Minimal context required — just a source for index, just a query string for query.

---

## Scenario 1: AI Coding Assistant — Codebase Understanding

A coding assistant's backend process links against Vectorless directly. No network hop, latency-sensitive.

```rust
use vectorless::Engine;

let engine = Engine::builder()
    .workspace("./codebase_index")
    .build()?;

// Index project documentation
engine.index(IndexContext::new("./project/docs/"))?;
engine.index(IndexContext::new("./project/README.md"))?;

// Query — engine returns reasoning chain + relevant content
let result = engine.query(QueryContext::new("认证模块的 token 刷新逻辑在哪？"))?;

// Reasoning Chain:
//   1. Located docs/auth/rfc-003.md → "Token Lifecycle" section
//   2. Cross-reference detected → tracked to src/middleware/refresh.rs docs
//   3. Returns both sections + reasoning path

// Feed result into LLM for final answer
llm.chat(result.context(), "认证模块的 token 刷新逻辑在哪？");
```

---

## Scenario 2: Enterprise Knowledge Base — RAG Pipeline

An enterprise AI platform has its own HTTP layer, auth system, and user management. Vectorless handles retrieval only.

```python
import vectorless

engine = vectorless.Engine(
    workspace="./knowledge_base",
    summary_model="gpt-4o-mini",
    retrieval_model="gpt-4o"
)

# Batch index enterprise documents
engine.index(vectorless.IndexContext("policies/"))

# Retrieval function — plug into any chat framework
def retrieve_context(query: str) -> str:
    result = engine.query(vectorless.QueryContext(query, max_tokens=4000))
    return result.content  # Feed directly to LLM
```

---

## Scenario 3: Legal Contract Review — Cross-Document Comparison

A legal review tool with its own UI and approval workflow. Vectorless provides precise clause location and cross-document reasoning.

```python
engine = vectorless.Engine()
engine.index(vectorless.IndexContext("contract_v1.docx"))
engine.index(vectorless.IndexContext("contract_v2.docx"))

# Cross-document reasoning
result = engine.query(vectorless.QueryContext("v2 中关于违约责任的条款有哪些变化？"))

# Reasoning Chain:
#   1. Located "违约责任" section in v2 (Section 8)
#   2. Cross-document tracking → found corresponding section in v1
#   3. Comparison → returned diff content

# Scoped query — restrict to single document
content = engine.query(
    vectorless.QueryContext("详细条款内容", scope="contract_v2.docx")
)
```

---

## Scenario 4: Academic Paper Research Assistant

Researchers use Vectorless in Jupyter notebooks or CLI tools. Python SDK integrates directly.

```python
engine = vectorless.Engine()

for paper in arxiv_papers:  # Downloaded as PDFs
    engine.index(vectorless.IndexContext(paper))

# Complex query — automatic decomposition
result = engine.query(
    vectorless.QueryContext("这些论文中，Transformer 架构在时序预测任务上的改进方向有哪些？")
)

# result.reasoning_chain shows:
#   Paper A → Section 3.2: Attention mechanism improvements
#   Paper C → Section 5:   Loss function optimization
#   Paper F → Section 2.1: Hybrid architecture with LSTM
#   [Auditable, traceable reasoning path]

# Follow-up — engine maintains context awareness
result2 = engine.query(
    vectorless.QueryContext("展开讲讲 Paper C 的损失函数优化，和传统 MSE 相比有什么优势？")
)
# → Auto-locates Paper C Section 5, deep dive
```

---

## Scenario 5: Technical Documentation Search — Embedded in Build Tools

Compiled into the binary of a static site generator. No separate service needed. Zero dependencies.

```rust
use vectorless::{Engine, IndexContext, QueryContext};

let engine = Engine::builder()
    .workspace("./docs_index")
    .build()?;

// Index at build time
engine.index(IndexContext::new("./docs/content/"))?;

// Called by mdbook/zola/other static site generators
fn search(query: &str) -> Vec<SearchResult> {
    let result = engine.query(QueryContext::new(query))?;
    result.entries.iter()
        .map(|e| SearchResult {
            title: e.title.clone(),
            section: e.path.clone(),        // "3.2.1 Configuration Options"
            snippet: e.content.clone(),
            score: e.confidence,
        })
        .collect()
}
```

---

## Scenario 6: CLI Tool — Local Developer Knowledge Base

Someone builds a CLI tool on top of Vectorless. The engine doesn't care about the interface — it just provides retrieval capability.

```bash
# Built on Vectorless by a third party

vls index ./project-docs/
vls query "部署流程中数据库迁移的步骤是什么"

# Output:
# docs/deploy.md > Section 3: Database Migration
#    1. Backup production database (pg_dump)
#    2. Run migration scripts (migrate up)
#    3. Verify schema version
#
# docs/runbook.md > Section 2.1: Rollback Plan
#    If migration fails...
#
# Reasoning: deploy.md:3 → runbook.md:2.1 (cross-reference tracking)
```

---

## Engine Boundary

```
┌─────────────────────────────────────────────────┐
│              User's Application Layer            │
│  HTTP API / CLI / GUI / Jupyter / Chat UI       │  ← Not Vectorless
├─────────────────────────────────────────────────┤
│              Vectorless Engine                   │
│                                                 │
│  Rust Library ──────── Python SDK (PyO3)        │
│                                                 │
│  index(IndexContext)                            │
│  query(QueryContext)                            │
│  + Reasoning Chain                              │
│  + Document Graph                               │
│  + Pre-computed Index                           │
│  + Tiered Cache                                 │
│  + Adaptive Token Budget                        │
└─────────────────────────────────────────────────┘
```

**Vectorless promises: the most relevant content + why it's relevant. The rest is up to you.**
