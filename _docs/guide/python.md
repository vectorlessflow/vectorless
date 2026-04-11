# Python Usage Guide

## Installation

```bash
pip install vectorless
```

## Configuration

### Zero Configuration

Set your API key and you're ready:

```bash
export OPENAI_API_KEY="sk-..."
```

```python
from vectorless import Engine, IndexContext

engine = Engine(workspace="./data")
```

### Custom Settings

```python
engine = Engine(
    workspace="./data",
    api_key="sk-...",
    model="gpt-4o",
    endpoint="https://api.openai.com/v1",
)
```

### Configuration File

For fine-grained control, use a config file:

```bash
cp vectorless.example.toml ./vectorless.toml
```

```python
engine = Engine(config_path="./vectorless.toml")
```

**Configuration priority** (highest → lowest):
1. Constructor parameters (`api_key`, `model`, `endpoint`)
2. Environment variables (`OPENAI_API_KEY`, `VECTORLESS_MODEL`, etc.)
3. Explicit config file (`config_path`)
4. Auto-detected config file (`vectorless.toml`, `config.toml`)
5. Default configuration

## Indexing

### From File

Supports PDF, Markdown, DOCX, HTML:

```python
doc_id = engine.index(IndexContext.from_file("./report.pdf"))
doc_id = engine.index(IndexContext.from_file("./readme.md"))
doc_id = engine.index(IndexContext.from_file("./doc.docx"))
doc_id = engine.index(IndexContext.from_file("./page.html"))
```

### From Content

Index from a string:

```python
ctx = IndexContext.from_content(
    content="# Manual\n## Chapter 1\nIntroduction...",
    name="manual",
    format="markdown",
)
doc_id = engine.index(ctx)
```

Supported formats: `"markdown"`, `"html"`, `"text"`

### From Bytes

Index from binary data (PDF, DOCX):

```python
with open("./report.pdf", "rb") as f:
    data = f.read()

ctx = IndexContext.from_bytes(data, name="report", format="pdf")
doc_id = engine.index(ctx)
```

## Querying

### Basic Query

```python
result = engine.query(doc_id, "What is the total revenue?")

print(result.content)   # Retrieved content
print(result.score)     # Relevance score (0.0 - 1.0)
print(result.doc_id)    # Document ID
print(result.node_ids)  # Matched node IDs
```

### Multiple Queries

```python
questions = [
    "What are the main components?",
    "How does the retrieval pipeline work?",
    "What is the architecture?",
]

for q in questions:
    result = engine.query(doc_id, q)
    print(f"Q: {q}")
    print(f"A: {result.content}")
    print(f"Score: {result.score:.2f}\n")
```

## Document Management

### List Documents

```python
for doc in engine.list_docs():
    print(f"{doc.id}: {doc.name} ({doc.format})")
```

### Check Existence

```python
if engine.exists(doc_id):
    print("Found")
```

### Remove Document

```python
engine.remove(doc_id)
```

### Clear All

```python
count = engine.clear()
print(f"Removed {count} documents")
```

### Get Page Content (PDF)

```python
# Single page
content = engine.get_page_content(doc_id, "1")

# Page range
content = engine.get_page_content(doc_id, "1-5")

# Multiple pages
content = engine.get_page_content(doc_id, "1,3,7")
```

## Environment Variables

| Variable | Description |
|----------|-------------|
| `OPENAI_API_KEY` | LLM API key |
| `VECTORLESS_MODEL` | Default model (e.g., `gpt-4o-mini`) |
| `VECTORLESS_ENDPOINT` | Custom API endpoint URL |
| `VECTORLESS_WORKSPACE` | Workspace directory |

## Error Handling

```python
from vectorless import VectorlessError

try:
    result = engine.query(doc_id, "question")
except VectorlessError as e:
    print(f"Error: {e.message}")
    print(f"Kind: {e.kind}")  # "not_found", "parse", "config", "workspace", "llm", "unknown"
```

## Examples

See [examples/python/](../../examples/python/) for complete examples:

- **basic/** — Zero configuration, simplest usage
- **advanced/** — Full configuration file with all options
- **custom_config/** — Custom API key, model, and endpoint
