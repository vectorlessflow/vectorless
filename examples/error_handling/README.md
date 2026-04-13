# Error Handling Example

Demonstrates how to catch and inspect `VectorlessError` exceptions:

- Invalid format strings
- Invalid indexing modes
- Querying non-existent documents
- Batch indexing with partial failures
- Engine creation with invalid credentials

The `VectorlessError` exception provides:
- `kind` -- error category (`"config"`, `"not_found"`, `"parse"`, `"llm"`, etc.)
- `message` -- human-readable error description

## Setup

```bash
pip install vectorless
```

## Run

```bash
python main.py
```

## Environment Variables

| Variable                | Description          | Default   |
|------------------------|----------------------|-----------|
| `VECTORLESS_API_KEY`   | LLM API key          | `sk-...`  |
| `VECTORLESS_MODEL`     | LLM model name       | `gpt-4o`  |
| `VECTORLESS_ENDPOINT`  | Custom API endpoint  | `None`    |
