# Batch Indexing Example

Demonstrates indexing multiple documents at once using:
- `from_paths` -- explicit list of file paths
- `from_dir` -- all supported files in a directory
- `from_bytes` -- raw in-memory content

Also shows cross-document querying with `with_doc_ids`.

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
