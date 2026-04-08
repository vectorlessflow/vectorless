#!/usr/bin/env python3
# Copyright (c) 2026 vectorless developers
# SPDX-License-Identifier: Apache-2.0

"""
Custom configuration example - Using your own API key, model, and endpoint.

This example demonstrates how to use custom LLM settings without a config file.
Useful when you want to use different providers like Azure OpenAI, DeepSeek, etc.

Usage:
    python custom_config.py
"""

import os
import tempfile
from vectorless import Engine, IndexContext


def main():
    print("=== Vectorless Custom Configuration Example ===\n")

    # ============================================================
    # Option 1: Use environment variables
    # ============================================================
    # Set these environment variables:
    # - OPENAI_API_KEY or VECTORLESS_API_KEY
    # - VECTORLESS_MODEL (optional)
    # - VECTORLESS_ENDPOINT (optional)

    # ============================================================
    # Option 2: Use constructor parameters (recommended for custom config)
    # ============================================================

    with tempfile.TemporaryDirectory() as workspace:
        # Example: Use DeepSeek API
        engine = Engine(
            workspace=workspace,
            api_key="sk-your-deepseek-key",  # Your API key
            model="deepseek-chat",            # Model name
            endpoint="https://api.deepseek.com/v1",  # API endpoint
        )

        print("✓ Engine created with custom settings\n")

        # Index a document
        content = """
# Product Documentation

## Overview
This product helps you manage documents intelligently.

## Features
- Fast indexing
- Accurate retrieval
- Easy to use API

## Installation
Install with pip: pip install vectorless
"""
        ctx = IndexContext.from_text(content, name="docs", format="markdown")
        doc_id = engine.index(ctx)
        print(f"✓ Indexed: {doc_id}\n")

        # Query
        result = engine.query(doc_id, "How do I install the product?")
        print("Query: How do I install the product?")
        print(f"Score: {result.score:.2f}")
        print(f"Result: {result.content[:200]}...\n")

        # Cleanup
        engine.remove(doc_id)
        print("✓ Cleaned up")

    # ============================================================
    # Other provider examples (commented out)
    # ============================================================

    # Azure OpenAI:
    # engine = Engine(
    #     workspace="./data",
    #     api_key="your-azure-key",
    #     model="gpt-4o",
    #     endpoint="https://your-resource.openai.azure.com/openai/deployments/your-deployment",
    # )

    # Local LLM (e.g., Ollama with OpenAI-compatible API):
    # engine = Engine(
    #     workspace="./data",
    #     model="llama3",
    #     endpoint="http://localhost:11434/v1",
    #     # No api_key needed for local LLM
    # )

    # Anthropic Claude (via OpenAI-compatible proxy):
    # engine = Engine(
    #     workspace="./data",
    #     api_key="sk-ant-...",
    #     model="claude-3-5-sonnet-20241022",
    #     endpoint="https://api.anthropic.com/v1",
    # )

    print("\n=== Done ===")


if __name__ == "__main__":
    main()
