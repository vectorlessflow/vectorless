#!/usr/bin/env python3
# Copyright (c) 2026 vectorless developers
# SPDX-License-Identifier: Apache-2.0

"""
Basic example demonstrating the vectorless Python library.

This example shows:
1. Creating an Engine with workspace
2. Indexing documents from different sources
3. Querying indexed documents
4. Managing documents (list, exists, remove)

Prerequisites:
    pip install vectorless
    export OPENAI_API_KEY="sk-..."

Usage:
    python python_basic.py
"""

import os
import tempfile
from pathlib import Path

from vectorless import Engine, IndexContext, VectorlessError


def main():
    # Create a temporary workspace for this example
    with tempfile.TemporaryDirectory() as workspace:
        print(f"Workspace: {workspace}")
        print()

        # ============================================================
        # 1. Create Engine
        # ============================================================
        print("=" * 60)
        print("1. Creating Engine")
        print("=" * 60)

        # Option A: Use OPENAI_API_KEY environment variable
        engine = Engine(workspace=workspace)

        # Option B: Explicit API key
        # engine = Engine(
        #     workspace=workspace,
        #     api_key="sk-...",
        #     model="gpt-4o-mini",  # optional
        # )

        print(f"Engine created successfully!")
        print(f"Initial document count: {engine.len()}")
        print()

        # ============================================================
        # 2. Index Documents
        # ============================================================
        print("=" * 60)
        print("2. Indexing Documents")
        print("=" * 60)

        # 2a. Index from text content (Markdown)
        markdown_content = """
# Technical Manual

## Chapter 1: Introduction

This document describes the architecture of our system.

## Chapter 2: Installation

### System Requirements

- Python 3.9+
- Rust 1.75+

### Steps

1. Install dependencies
2. Configure environment
3. Run the application

## Chapter 3: API Reference

### Engine

The main entry point for vectorless.

```python
engine = Engine(workspace="./data")
```

### IndexContext

Context for indexing documents from various sources.
"""
        ctx_md = IndexContext.from_text(
            markdown_content,
            name="technical_manual",
            format="markdown"
        )
        doc_id_md = engine.index(ctx_md)
        print(f"Indexed markdown document: {doc_id_md}")

        # 2b. Index from text content (HTML)
        html_content = """
<html>
<head><title>Product Guide</title></head>
<body>
    <h1>Product Guide</h1>
    <h2>Getting Started</h2>
    <p>Welcome to our product. This guide will help you get started.</p>
    <h2>Features</h2>
    <ul>
        <li>Fast indexing</li>
        <li>Accurate retrieval</li>
        <li>Easy to use API</li>
    </ul>
</body>
</html>
"""
        ctx_html = IndexContext.from_text(
            html_content,
            name="product_guide",
            format="html"
        )
        doc_id_html = engine.index(ctx_html)
        print(f"Indexed HTML document: {doc_id_html}")

        # 2c. Index from text content (plain text)
        text_content = """
Meeting Notes - Q4 Planning

Date: 2024-01-15

Attendees: Alice, Bob, Charlie

Agenda:
1. Review Q3 performance
2. Set Q4 goals
3. Resource allocation

Key Decisions:
- Increase marketing budget by 20%
- Launch new product in March
- Hire 5 additional engineers
"""
        ctx_text = IndexContext.from_text(
            text_content,
            name="meeting_notes",
            format="text"
        )
        doc_id_text = engine.index(ctx_text)
        print(f"Indexed text document: {doc_id_text}")

        # 2d. Index from file (if you have actual files)
        # ctx_file = IndexContext.from_file("./report.pdf")
        # doc_id_file = engine.index(ctx_file)
        # print(f"Indexed file: {doc_id_file}")

        print(f"\nTotal documents indexed: {engine.len()}")
        print()

        # ============================================================
        # 3. List Documents
        # ============================================================
        print("=" * 60)
        print("3. Listing Documents")
        print("=" * 60)

        docs = engine.list_docs()
        for doc in docs:
            print(f"  - {doc.name} (id: {doc.id}, format: {doc.format})")
            if doc.line_count:
                print(f"    Lines: {doc.line_count}")
        print()

        # ============================================================
        # 4. Query Documents
        # ============================================================
        print("=" * 60)
        print("4. Querying Documents")
        print("=" * 60)

        # Query the technical manual
        questions = [
            "What are the system requirements?",
            "How do I create an Engine?",
            "What are the installation steps?",
        ]

        for question in questions:
            result = engine.query(doc_id_md, question)
            print(f"Q: {question}")
            print(f"A: {result.content[:200]}...")
            print(f"   Score: {result.score:.2f}")
            print()

        # Query the meeting notes
        result = engine.query(doc_id_text, "What was decided about the marketing budget?")
        print(f"Q: What was decided about the marketing budget?")
        print(f"A: {result.content}")
        print(f"   Score: {result.score:.2f}")
        print()

        # ============================================================
        # 5. Check Document Existence
        # ============================================================
        print("=" * 60)
        print("5. Checking Document Existence")
        print("=" * 60)

        print(f"Document {doc_id_md[:8]}... exists: {engine.exists(doc_id_md)}")
        print(f"Document 'nonexistent' exists: {engine.exists('nonexistent')}")
        print()

        # ============================================================
        # 6. Error Handling
        # ============================================================
        print("=" * 60)
        print("6. Error Handling")
        print("=" * 60)

        try:
            engine.query("nonexistent_doc_id", "question")
        except VectorlessError as e:
            print(f"Caught error: {e.message}")
            print(f"Error kind: {e.kind}")
        print()

        # ============================================================
        # 7. Remove Documents
        # ============================================================
        print("=" * 60)
        print("7. Removing Documents")
        print("=" * 60)

        # Remove the HTML document
        removed = engine.remove(doc_id_html)
        print(f"Removed {doc_id_html}: {removed}")
        print(f"Documents remaining: {engine.len()}")

        # Try to remove again (should return False)
        removed_again = engine.remove(doc_id_html)
        print(f"Remove again: {removed_again}")
        print()

        # ============================================================
        # 8. Clear All Documents
        # ============================================================
        print("=" * 60)
        print("8. Clearing All Documents")
        print("=" * 60)

        cleared_count = engine.clear()
        print(f"Cleared {cleared_count} documents")
        print(f"Final document count: {engine.len()}")
        print()

        print("=" * 60)
        print("Example completed successfully!")
        print("=" * 60)


if __name__ == "__main__":
    # Check for API key
    if not os.environ.get("OPENAI_API_KEY"):
        print("Warning: OPENAI_API_KEY environment variable not set.")
        print("Some operations may fail without an API key.")
        print()

    main()
