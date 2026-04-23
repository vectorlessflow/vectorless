# Vectorless Python SDK

Python bindings for [vectorless](https://github.com/vectorlessflow/vectorless) — a Document Understanding Engine for AI.

## Installation

```bash
pip install vectorless
```

## Quick Start

```python
import asyncio
from vectorless import Engine

async def main():
    # Create engine — api_key and model are required
    engine = Engine(
        api_key="sk-...",
        model="gpt-4o",
    )

    # Understand a document
    doc = await engine.ingest("./report.pdf")
    print(f"Understood: {doc.name} — {doc.summary}")

    # Ask a question
    answer = await engine.ask(
        "What is the total revenue?",
        doc_ids=[doc.doc_id],
    )
    print(f"Answer: {answer.content}")
    print(f"Confidence: {answer.confidence:.2f}")
    print(f"Evidence: {len(answer.evidence)} pieces")
    print(f"Trace: {len(answer.trace.steps)} steps")

    # List all understood documents
    docs = await engine.list_documents()
    for d in docs:
        print(f"  - {d.name} ({d.doc_id})")

    # Forget a document
    await engine.forget(doc.doc_id)

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
        config: Config | None = None,
    ): ...

    async def ingest(self, path: str) -> DocumentInfo: ...
    async def ask(self, question: str, doc_ids: list[str] | None = None) -> Answer: ...
    async def forget(self, doc_id: str) -> None: ...
    async def list_documents(self) -> list[DocumentInfo]: ...
    async def exists(self, doc_id: str) -> bool: ...
    async def clear(self) -> int: ...
    async def get_graph(self) -> DocumentGraph | None: ...
    def metrics_report(self) -> MetricsReport: ...
```

### DocumentInfo

```python
class DocumentInfo:
    doc_id: str
    name: str
    format: str
    summary: str
    concepts: list[Concept]
    section_count: int
    page_count: int | None
```

### Answer

```python
class Answer:
    content: str
    evidence: list[Evidence]
    confidence: float
    trace: ReasoningTrace
```

### Evidence

```python
class Evidence:
    content: str
    source_path: str
    doc_name: str
    relevance: float
```

### ReasoningTrace

```python
class ReasoningTrace:
    steps: list[TraceStep]
```

### TraceStep

```python
class TraceStep:
    action: str
    observation: str
    round: int
```

### Concept

```python
class Concept:
    name: str
    description: str
    confidence: float
```

### VectorlessError

```python
class VectorlessError(Exception):
    message: str
    kind: str  # "config", "parse", "not_found", "llm"
```

## Development

### Building from source

```bash
# Install maturin
pip install maturin

# Build and install (from project root)
maturin develop

# Run tests
pytest
```

### Publishing to PyPI

```bash
maturin build --release
maturin publish
```

## License

Apache-2.0
