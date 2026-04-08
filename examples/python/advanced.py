#!/usr/bin/env python3
# Copyright (c) 2026 vectorless developers
# SPDX-License-Identifier: Apache-2.0

"""
Advanced usage example - Full Configuration.

This example demonstrates how to use a full configuration file
for advanced use cases where you need fine-grained control.

Usage:
    # First, copy the example config
    cp config.toml ./my_vectorless.toml

    # Edit my_vectorless.toml to customize settings

    # Run the example
    python advanced.py
"""

import os
import tempfile
from vectorless import Engine, IndexContext


def main():
    print("=== Vectorless Advanced Example (Full Configuration) ===\n")

    # Method 1: Use explicit config file path
    # This loads all settings from the specified config file
    engine = Engine(config_path="./config.toml")

    print("✓ Engine created with config file\n")

    # Index a document
    content = """
# Product Documentation

## Overview

This is a comprehensive guide for our product.

## Configuration

The system supports multiple configuration methods:

### 1. Zero Configuration
Just set OPENAI_API_KEY environment variable.

### 2. Environment Variables
- VECTORLESS_MODEL: Set default model
- VECTORLESS_ENDPOINT: Set API endpoint
- VECTORLESS_WORKSPACE: Set workspace directory

### 3. Config File
Create a vectorless.toml file with full configuration.

## API Reference

### Engine
The main entry point for vectorless.

### IndexContext
Context for indexing documents from various sources.
"""
    ctx = IndexContext.from_text(content, name="docs", format="markdown")
    doc_id = engine.index(ctx)
    print(f"✓ Indexed: {doc_id}\n")

    # Query
    result = engine.query(doc_id, "What configuration methods are available?")
    print("Query: What configuration methods are available?")
    print(f"Score: {result.score:.2f}")
    print(f"Result: {result.content[:200]}...\n")

    # Cleanup
    engine.remove(doc_id)
    print("✓ Cleaned up")

    print("\n" + "=" * 60)
    print("Configuration Options")
    print("=" * 60)
    print()
    print("Configuration Priority (later overrides earlier):")
    print("  1. Default configuration")
    print("  2. Auto-detected config file (vectorless.toml, config.toml)")
    print("  3. Explicit config file (config_path parameter)")
    print("  4. Environment variables")
    print("  5. Constructor parameters (api_key, model, etc.)")
    print()
    print("Environment Variables:")
    print("  OPENAI_API_KEY       - LLM API key")
    print("  VECTORLESS_MODEL     - Default model name")
    print("  VECTORLESS_ENDPOINT  - API endpoint URL")
    print("  VECTORLESS_WORKSPACE - Workspace directory")
    print()
    print("Usage Examples:")
    print()
    print("# Zero configuration (recommended for beginners)")
    print('engine = Engine(workspace="./data")')
    print()
    print("# With custom model")
    print('engine = Engine(workspace="./data", model="gpt-4o-mini")')
    print()
    print("# With full config file (advanced)")
    print('engine = Engine(config_path="./vectorless.toml")')
    print()
    print("# Override config with parameters")
    print('engine = Engine(')
    print('    config_path="./vectorless.toml",')
    print('    model="gpt-4o",  # Override model from config')
    print(')')

    print("\n=== Done ===")


if __name__ == "__main__":
    main()
