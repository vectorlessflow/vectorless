# Session API Walkthrough

Demonstrates the full high-level Vectorless Python API using the `Session` and `SyncSession` classes.

## What it covers

| # | Topic | API |
|---|-------|-----|
| 1 | Session creation | `Session()`, `from_env()`, `from_config_file()` |
| 2 | Indexing sources | `index(content=)`, `index(path=)`, `index(bytes_data=)`, `index(directory=)` |
| 3 | Batch indexing | `index_batch(paths, jobs=N)` |
| 4 | Querying | `ask(question, doc_ids=)`, `ask(question, workspace_scope=True)` |
| 5 | Streaming query | `query_stream()` async iterator |
| 6 | Document management | `list_documents()`, `document_exists()`, `remove_document()`, `clear_all()` |
| 7 | Document graph | `get_graph()` nodes, edges, keywords |
| 8 | Event callbacks | `EventEmitter` with `@on_index` / `@on_query` decorators |
| 9 | Metrics | `metrics_report()` |
| 10 | Sync API | `SyncSession` (no async/await) |

## Setup

```bash
pip install vectorless
export VECTORLESS_API_KEY="sk-..."
export VECTORLESS_MODEL="gpt-4o"
```

## Run

```bash
python main.py
```
