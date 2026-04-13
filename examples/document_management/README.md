# Document Management Example

Demonstrates CRUD operations on indexed documents:

- `engine.list()` -- list all documents
- `engine.exists(doc_id)` -- check if a document exists
- `engine.remove(doc_id)` -- remove a single document
- `engine.clear()` -- remove all documents

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
