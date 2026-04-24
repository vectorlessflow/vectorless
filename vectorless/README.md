# Vectorless

Python bindings for [Vectorless](https://github.com/vectorlessflow/vectorless) — knowing by reasoning, not vectors.

## Installation

```bash
pip install -U vectorless
```

## Quick Start

```python
import asyncio
from vectorless import Engine

async def main():
    engine = Engine(api_key="sk-...", model="gpt-4o")

    # Compile a document
    result = await engine.compile(path="./report.pdf")
    doc_id = result.doc_id

    # Ask a question
    response = await engine.ask("What is the total revenue?", doc_ids=[doc_id])
    print(response.answer)
    print(f"Confidence: {response.confidence:.2f}")
    print(f"Evidence: {len(response.evidence)} pieces")

    # List all documents
    docs = await engine.list_documents()
    for d in docs:
        print(f"  - {d.name} ({d.doc_id})")

    # Remove a document
    await engine.remove_document(doc_id)

asyncio.run(main())
```

## API Reference

### Engine

The main entry point. All methods are **async**.

```python
class Engine:
    def __init__(
        self,
        api_key: str | None = None,
        model: str | None = None,
        endpoint: str | None = None,
        config: EngineConfig | None = None,
    ): ...

    async def compile(self, path: str | None = None, *, format: str | None = None, mode: str | None = None, force: bool = False) -> IndexResultWrapper: ...
    async def compile_batch(self, paths: list[str], *, mode: str | None = None, jobs: int = 4, force: bool = False) -> IndexResultWrapper: ...
    async def ask(self, question: str, doc_ids: list[str] | None = None, timeout_secs: int | None = None) -> Output: ...
    async def query_stream(self, question: str, doc_ids: list[str] | None = None) -> StreamingQueryResult: ...
    async def list_documents(self) -> list[DocCard]: ...
    async def remove_document(self, doc_id: str) -> None: ...
    async def document_exists(self, doc_id: str) -> bool: ...
    async def clear_all(self) -> int: ...
    async def get_graph(self) -> DocumentGraph | None: ...
    def metrics_report(self) -> MetricsReport: ...
```

### IndexResultWrapper

```python
class IndexResultWrapper:
    doc_id: str | None
    items: list[DocCard]
    failed: list[FailedItem]
```

### Output

```python
class Output:
    answer: str
    evidence: list[Evidence]
    confidence: float
    trace_steps: list[TraceStep]
    metrics: QueryMetrics
```

### Evidence

```python
class Evidence:
    content: str
    source_path: str
    node_title: str
    doc_name: str
```

### DocCard

```python
class DocCard:
    doc_id: str
    name: str
    summary: str
    section_count: int
    concepts: list[Concept]
```

### TraceStep

```python
class TraceStep:
    action: str
    observation: str
    round: int
```

### VectorlessError

```python
class VectorlessError(Exception):
    message: str
    kind: str  # "config", "parse", "not_found", "llm"
```

## Development

```bash
pip install maturin
maturin develop      # Build and install
pytest               # Run tests
```

## License

Apache-2.0
