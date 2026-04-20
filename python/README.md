# Vectorless Python SDK

Python bindings for [vectorless](https://github.com/vectorlessflow/vectorless) — a reasoning-native document intelligence engine for AI.

## Installation

```bash
pip install vectorless
```

## Quick Start

```python
import asyncio
from vectorless import Engine, IndexContext, QueryContext

async def main():
    # Create engine — api_key and model are required
    engine = Engine(
        api_key="sk-...",
        model="gpt-4o",
    )

    # Index a document
    result = await engine.index(IndexContext.from_path("./report.pdf"))
    doc_id = result.doc_id
    print(f"Indexed: {doc_id}")

    # Query the document
    result = await engine.query(
        QueryContext("What is the total revenue?").with_doc_ids([doc_id])
    )
    item = result.single()
    print(f"Answer: {item.content}")
    print(f"Score: {item.score:.2f}")

    # List all documents
    for doc in await engine.list():
        print(f"  - {doc.name} ({doc.id})")

    # Cleanup
    await engine.remove(doc_id)

asyncio.run(main())
```

## API Reference

### Engine

The main entry point for vectorless.

```python
class Engine:
    def __init__(
        self,
        config_path: str | None = None,
        api_key: str | None = None,
        model: str | None = None,
        endpoint: str | None = None,
    ): ...

    async def index(self, ctx: IndexContext) -> IndexResult: ...
    async def query(self, ctx: QueryContext) -> QueryResult: ...
    async def list(self) -> list[DocumentInfo]: ...
    async def remove(self, doc_id: str) -> bool: ...
    async def clear(self) -> int: ...
    async def exists(self, doc_id: str) -> bool: ...
    async def get_graph(self) -> DocumentGraph | None: ...
```

### IndexContext

Context for indexing documents.

```python
class IndexContext:
    @staticmethod
    def from_path(path: str, name: str | None = None) -> IndexContext: ...

    @staticmethod
    def from_paths(paths: list[str]) -> IndexContext: ...

    @staticmethod
    def from_dir(path: str, recursive: bool = True) -> IndexContext: ...

    @staticmethod
    def from_content(
        content: str,
        name: str | None = None,
        format: str = "markdown",
    ) -> IndexContext: ...

    @staticmethod
    def from_bytes(
        data: bytes,
        name: str,
        format: str,
    ) -> IndexContext: ...

    def with_options(self, options: IndexOptions) -> IndexContext: ...
    def with_mode(self, mode: str) -> IndexContext: ...
```

**Supported formats:**
- `"markdown"` / `"md"` - Markdown content
- `"pdf"` - PDF documents

### QueryContext

Context for querying documents.

```python
class QueryContext:
    def __init__(self, query: str): ...

    def with_doc_ids(self, doc_ids: list[str]) -> QueryContext: ...
    def with_workspace(self) -> QueryContext: ...
    def with_timeout_secs(self, secs: int) -> QueryContext: ...
    def with_force_analysis(self, force: bool) -> QueryContext: ...
```

### IndexResult

```python
class IndexResult:
    @property
    def doc_id(self) -> str | None: ...
    @property
    def items(self) -> list[IndexItem]: ...
    @property
    def failed(self) -> list[FailedItem]: ...
    def has_failures(self) -> bool: ...
    def total(self) -> int: ...
    def __len__(self) -> int: ...
```

### QueryResult

```python
class QueryResult:
    @property
    def items(self) -> list[QueryResultItem]: ...
    @property
    def failed(self) -> list[FailedItem]: ...
    def single(self) -> QueryResultItem | None: ...
    def has_failures(self) -> bool: ...
    def __len__(self) -> int: ...
```

### QueryResultItem

```python
class QueryResultItem:
    @property
    def doc_id(self) -> str: ...
    @property
    def content(self) -> str: ...
    @property
    def score(self) -> float: ...
    @property
    def node_ids(self) -> list[str]: ...
```

### IndexItem

```python
class IndexItem:
    @property
    def doc_id(self) -> str: ...
    @property
    def name(self) -> str: ...
    @property
    def format(self) -> str: ...
    @property
    def description(self) -> str | None: ...
    @property
    def source_path(self) -> str | None: ...
    @property
    def page_count(self) -> int | None: ...
    @property
    def metrics(self) -> IndexMetrics | None: ...
```

### DocumentInfo

```python
class DocumentInfo:
    @property
    def id(self) -> str: ...
    @property
    def name(self) -> str: ...
    @property
    def format(self) -> str: ...
    @property
    def description(self) -> str | None: ...
    @property
    def source_path(self) -> str | None: ...
    @property
    def page_count(self) -> int | None: ...
    @property
    def line_count(self) -> int | None: ...
```

### VectorlessError

```python
class VectorlessError(Exception):
    @property
    def message(self) -> str: ...
    @property
    def kind(self) -> str: ...  # "config", "parse", "not_found", "llm"
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
