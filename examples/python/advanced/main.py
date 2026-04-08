#!/usr/bin/env python3
"""
Advanced example - Full Configuration File.

This example demonstrates how to use a full configuration file
for fine-grained control over all settings.

Usage:
    cp ../../../config.toml ./vectorless.toml
    # Edit vectorless.toml to customize settings
    python main.py
"""

import os
from vectorless import Engine, IndexContext

# Path to config file (relative to this script)
CONFIG_PATH = "./vectorless.toml"
WORKSPACE = "./workspace"


def main():
    print("=== Vectorless Advanced Example (Full Configuration) ===\n")

    # Check if config file exists
    if not os.path.exists(CONFIG_PATH):
        print(f"Error: Config file not found: {CONFIG_PATH}")
        print("\nCreate it by copying the example:")
        print(f"  cp ../../../config.toml {CONFIG_PATH}")
        print("\nThen edit it to customize your settings.")
        return

    # Create engine with config file
    engine = Engine(config_path=CONFIG_PATH)

    print(f"✓ Engine created with config file: {CONFIG_PATH}\n")

    # Index a document
    content = """
# System Documentation

## Architecture

The system consists of three main components:

1. **Index Pipeline** - Parses documents and builds a navigable tree
2. **Retrieval Pipeline** - Queries and retrieves relevant content
3. **Pilot** - LLM-powered navigation guide

## Configuration Options

### LLM Settings
- `model`: The LLM model to use (e.g., "gpt-4o", "gpt-4o-mini")
- `endpoint`: API endpoint URL
- `api_key`: Your API key
- `temperature`: Generation temperature (0.0 for deterministic)

### Retrieval Settings
- `top_k`: Number of results to return
- `max_iterations`: Maximum search iterations
- `beam_width`: Beam width for multi-path search

### Storage Settings
- `workspace_dir`: Directory for persisted documents
- `cache_size`: LRU cache size
- `compression`: Enable/disable compression

## Performance Tuning

For faster retrieval:
- Use a smaller model like gpt-4o-mini
- Reduce max_iterations
- Enable caching

For higher accuracy:
- Use a more capable model like gpt-4o
- Increase beam_width
- Enable multi-turn decomposition
"""
    ctx = IndexContext.from_content(content, name="system_docs", format="markdown")
    doc_id = engine.index(ctx)
    print(f"✓ Indexed: {doc_id}\n")

    # Query examples
    questions = [
        "What are the main components?",
        "How can I improve retrieval speed?",
        "What settings are available?",
    ]

    for q in questions:
        result = engine.query(doc_id, q)
        print(f"Q: {q}")
        print(f"A: {result.content[:150]}...")
        print(f"   Score: {result.score:.2f}\n")

    # Cleanup
    engine.remove(doc_id)
    print("✓ Cleaned up")

    # Print configuration info
    print("\n" + "=" * 60)
    print("Configuration Priority")
    print("=" * 60)
    print("""
1. Default configuration
2. Auto-detected config file (vectorless.toml, config.toml)
3. Explicit config file (config_path parameter)
4. Environment variables (OPENAI_API_KEY, etc.)
5. Constructor parameters (api_key, model, etc.)
""")


if __name__ == "__main__":
    main()
