"""
Indexing example — demonstrates the full Vectorless workflow.

Usage:
    pip install vectorless
    python main.py
"""

import asyncio
import os
from vectorless import Engine, IndexContext, IndexOptions, QueryContext

# os is used only for removing the sample file

# --- Configuration ---
# Replace with your own credentials
API_KEY = "sk-..."
MODEL = "gpt-4o"


async def main():
    # --- 1. Create engine ---
    engine = Engine(
        api_key=API_KEY,
        model=MODEL,
    )
    print("Engine created\n")

    # --- 2. Index from text ---
    print("--- Index from text ---")
    result = await engine.index(
        IndexContext.from_content(
            """# Architecture Guide

## Overview

Vectorless is a reasoning-native document intelligence engine.
It uses hierarchical semantic trees instead of vector embeddings.

## Key Concepts

- **Semantic Tree**: Documents are parsed into a tree of sections.
- **LLM Navigation**: Queries are resolved by traversing the tree.
- **No Vectors**: No embeddings, no similarity search, no vector DB.
""",
            "markdown",
        ).with_name("architecture")
    )
    doc_id = result.doc_id
    print(f"  Indexed: {doc_id}")
    print(f"  Items: {result.total()}\n")

    # --- 3. Index from file ---
    print("--- Index from file ---")
    # Write a sample file first
    sample_path = "./sample_report.md"
    with open(sample_path, "w") as f:
        f.write("""# Q4 Financial Report

## Revenue

Total revenue for Q4 was $12.3M, up 15% from Q3.
SaaS subscriptions accounted for $8.1M, consulting for $4.2M.

## Costs

Operating costs were $9.8M, including $3.2M in engineering salaries.
Marketing spend was reduced by 8% to $1.5M.

## Outlook

Projected Q1 revenue is $13.5M based on current pipeline.
""")

    result = await engine.index(IndexContext.from_path(sample_path))
    file_doc_id = result.doc_id
    print(f"  Indexed: {file_doc_id}\n")
    os.remove(sample_path)

    # --- 4. Index with options ---
    print("--- Index with options (summaries + description) ---")
    result = await engine.index(
        IndexContext.from_content(
            "# API Reference\n\n## GET /users\n\nList all users.\n\n## POST /users\n\nCreate a user.",
            "markdown",
        )
        .with_name("api_ref")
        .with_options(IndexOptions(generate_summaries=True, generate_description=True)),
    )
    print(f"  Indexed: {result.doc_id}\n")

    # --- 5. Query ---
    print("--- Query ---")
    answer = await engine.query(
        QueryContext("What was the total revenue?").with_doc_id(file_doc_id)
    )
    item = answer.single()
    if item:
        print(f"  Score: {item.score:.2f}")
        print(f"  Answer: {item.content[:200]}\n")

    # --- 6. List documents ---
    print("--- List documents ---")
    docs = await engine.list()
    for doc in docs:
        desc = f" — {doc.description}" if doc.description else ""
        print(f"  {doc.name} ({doc.id[:8]}...){desc}")
    print()

    # --- 7. Document graph ---
    print("--- Document graph ---")
    graph = await engine.get_graph()
    if graph:
        print(f"  Nodes: {graph.node_count()}, Edges: {graph.edge_count()}")
        for doc_id in graph.doc_ids():
            node = graph.get_node(doc_id)
            if node:
                neighbors = graph.get_neighbors(doc_id)
                kw = ", ".join(k.keyword for k in node.top_keywords[:3])
                print(f"  {node.title}: keywords=[{kw}], neighbors={len(neighbors)}")
    print()

    # --- 8. Cleanup ---
    print("--- Cleanup ---")
    removed = await engine.clear()
    print(f"  Removed {removed} document(s)")


if __name__ == "__main__":
    asyncio.run(main())
