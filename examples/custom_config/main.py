#!/usr/bin/env python3
"""
Custom configuration example - Using your own API key, model, and endpoint.

This example demonstrates how to use custom LLM settings without a config file.
Useful when you want to use different providers like DeepSeek, Azure OpenAI, etc.

Usage:
    python main.py
"""

import tempfile
from vectorless import Engine, IndexContext

# ============================================================
# Configure your settings here
# ============================================================
API_KEY = "sk-or-v1-xxxx"  # Your API key
MODEL = "google/gemini-3-flash-preview"  # Model name
ENDPOINT = "https://api/v1"  # API endpoint
WORKSPACE = "./workspace"  # Workspace directory


def main():
    print("=== Vectorless Custom Configuration Example ===\n")

    # Create engine with custom settings
    engine = Engine(
        workspace=WORKSPACE,
        api_key=API_KEY,
        model=MODEL,
        endpoint=ENDPOINT,
    )

    print(f"✓ Engine created with custom settings")
    print(f"  Model: {MODEL}")
    print(f"  Endpoint: {ENDPOINT}\n")

    # Index a document
    content = """
# Product Documentation

## Overview
This product helps you manage documents intelligently using LLM-powered navigation.

## Features
- Fast indexing with tree-based structure
- Accurate retrieval using hybrid search
- Easy to use Python and Rust APIs
- Support for PDF, Markdown, HTML, and DOCX

## Installation

Install with pip:
```bash
pip install vectorless
```

## Quick Start

```python
from vectorless import Engine, IndexContext

# Create engine
engine = Engine(workspace="./data")

# Index a document
ctx = IndexContext.from_file("./report.pdf")
doc_id = engine.index(ctx)

# Query
result = engine.query(doc_id, "What is the total revenue?")
print(result.content)
```

## Configuration

Vectorless supports multiple configuration methods:
1. Zero configuration - just set OPENAI_API_KEY
2. Custom settings - pass api_key, model, endpoint
3. Full config file - use vectorless.toml
"""
    ctx = IndexContext.from_content(content, name="docs", format="markdown")
    doc_id = engine.index(ctx)
    print(f"✓ Indexed: {doc_id}\n")

    # Check document info
    docs = engine.list_docs()
    print(f"Documents in workspace: {len(docs)}")
    for d in docs:
        print(f"  - {d.name} (id: {d.id}, format: {d.format})")
    print()

    # Query
    result = engine.query(doc_id, "How do I install the product?")
    print("Query: How do I install the product?")
    if item := result.single():
        print(f"Score: {item.score:.2f}")
        print(f"Result: {item.content}\n")

    # Another query
    result = engine.query(doc_id, "What features are available?")
    print("Query: What features are available?")
    if item := result.single():
        print(f"Score: {item.score:.2f}")
        print(f"Result: {item.content}\n")

    # Cleanup
    engine.remove(doc_id)
    print("✓ Cleaned up")

    print("\n=== Done ===")


if __name__ == "__main__":
    main()
