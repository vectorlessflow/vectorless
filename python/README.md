# Vectorless Python Bindings

Python bindings for [vectorless](https://github.com/vectorlessflow/vectorless) - a hierarchical document intelligence engine.

## Installation

```bash
pip install vectorless
```

## Quick Start

```python
from vectorless import Engine, IndexContext

# Create engine (uses OPENAI_API_KEY env var by default)
engine = Engine(workspace="./data")

# Or with explicit API key
engine = Engine(workspace="./data", api_key="sk-...")

# Index a document
ctx = IndexContext.from_file("./report.pdf")
doc_id = engine.index(ctx)
print(f"Indexed: {doc_id}")

# Query the document
result = engine.query(doc_id, "What is the total revenue?")
print(f"Answer: {result.content}")
print(f"Score: {result.score:.2f}")

# List all documents
for doc in engine.list_docs():
    print(f"  - {doc.name} ({doc.id})")

# Cleanup
engine.remove(doc_id)
```

## API Reference

### Engine

The main entry point for vectorless.

```python
class Engine:
    def __init__(
        self,
        workspace: str,
        api_key: str | None = None,
        model: str | None = None,
        endpoint: str | None = None,
    ): ...

    def index(self, ctx: IndexContext) -> str: ...
    def query(self, doc_id: str, question: str) -> QueryResult: ...
    def list_docs(self) -> list[DocumentInfo]: ...
    def remove(self, doc_id: str) -> bool: ...
    def clear(self) -> int: ...
    def exists(self, doc_id: str) -> bool: ...
    def len(self) -> int: ...
```

### IndexContext

Context for indexing documents.

```python
class IndexContext:
    @staticmethod
    def from_file(path: str, name: str | None = None) -> IndexContext: ...

    @staticmethod
    def from_text(
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
```

**Supported formats:**
- `"markdown"` / `"md"` - Markdown content
- `"pdf"` - PDF documents
- `"docx"` / `"doc"` - Word documents
- `"html"` / `"htm"` - HTML content
- `"text"` / `"txt"` - Plain text

### QueryResult

Result of a document query.

```python
class QueryResult:
    @property
    def doc_id(self) -> str: ...

    @property
    def content(self) -> str: ...

    @property
    def score(self) -> float: ...

    @property
    def node_ids(self) -> list[str]: ...
```

### DocumentInfo

Information about an indexed document.

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
    def page_count(self) -> int | None: ...
    @property
    def line_count(self) -> int | None: ...
```

### VectorlessError

Exception raised for vectorless errors.

```python
class VectorlessError(Exception):
    @property
    def message(self) -> str: ...

    @property
    def kind(self) -> str: ...  # "config", "parse", "not_found", "llm"
```

## Examples

### Index from different sources

```python
from vectorless import Engine, IndexContext

engine = Engine(workspace="./data")

# From file (format auto-detected)
doc_id = engine.index(IndexContext.from_file("./report.pdf"))

# From markdown text
doc_id = engine.index(IndexContext.from_text(
    "# Report\n\nThis is the content...",
    name="report",
    format="markdown"
))

# From HTML
doc_id = engine.index(IndexContext.from_text(
    "<html><body><h1>Title</h1></body></html>",
    name="page",
    format="html"
))

# From bytes (e.g., downloaded file)
with open("document.pdf", "rb") as f:
    doc_id = engine.index(IndexContext.from_bytes(
        f.read(),
        name="downloaded",
        format="pdf"
    ))
```

### Error handling

```python
from vectorless import Engine, VectorlessError

engine = Engine(workspace="./data")

try:
    result = engine.query("nonexistent", "question")
except VectorlessError as e:
    print(f"Error: {e.message} (kind={e.kind})")
```

## Development

### Building from source

```bash
# Install maturin
pip install maturin

# Build and install
cd python
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
