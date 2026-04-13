# PDF Indexing Example

Demonstrates indexing a PDF file, inspecting indexing metrics, and querying.

## Setup

```bash
pip install vectorless
```

## Run

```bash
# Use the sample PDF from the repository
python main.py

# Or specify your own PDF file
python main.py /path/to/document.pdf
```

## Environment Variables

| Variable                | Description          | Default   |
|------------------------|----------------------|-----------|
| `VECTORLESS_API_KEY`   | LLM API key          | `sk-...`  |
| `VECTORLESS_MODEL`     | LLM model name       | `gpt-4o`  |
| `VECTORLESS_ENDPOINT`  | Custom API endpoint  | `None`    |
