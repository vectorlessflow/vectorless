#!/usr/bin/env python3
# Copyright (c) 2026 vectorless developers
# SPDX-License-Identifier: Apache-2.0

"""
Basic usage example - Zero Configuration.

This example demonstrates the simplest way to use Vectorless.
Just set OPENAI_API_KEY environment variable and you're ready to go.

Usage:
    export OPENAI_API_KEY="sk-..."
    python basic.py
"""

import os
import tempfile
from vectorless import Engine, IndexContext


def main():
    print("=== Vectorless Basic Example (Zero Configuration) ===\n")

    # Zero configuration: Just set OPENAI_API_KEY environment variable
    # The engine will automatically use it.
    with tempfile.TemporaryDirectory() as workspace:
        engine = Engine(workspace=workspace)

        print("✓ Engine created (using OPENAI_API_KEY from environment)\n")

        # Index from text content
        content = """
# Technical Manual

## Chapter 1: Introduction

Vectorless is a library for querying structured documents using natural language.

## Chapter 2: Installation

Install with pip:
```
pip install vectorless
```

## Chapter 3: Usage

```python
from vectorless import Engine, IndexContext

engine = Engine(workspace="./data")
ctx = IndexContext.from_file("./report.pdf")
doc_id = engine.index(ctx)

result = engine.query(doc_id, "What is the total revenue?")
print(result.content)
```
"""
        ctx = IndexContext.from_text(content, name="manual", format="markdown")
        doc_id = engine.index(ctx)
        print(f"✓ Indexed: {doc_id}\n")

        # Query
        result = engine.query(doc_id, "How do I install vectorless?")
        print("Query: How do I install vectorless?")
        print(f"Score: {result.score:.2f}")
        print(f"Result: {result.content[:200]}...\n")

        # Cleanup
        engine.remove(doc_id)
        print("✓ Cleaned up")

    print("\n=== Done ===")


if __name__ == "__main__":
    if not os.environ.get("OPENAI_API_KEY"):
        print("Error: OPENAI_API_KEY environment variable not set.")
        print("Set it with: export OPENAI_API_KEY='sk-...'")
        exit(1)

    main()
