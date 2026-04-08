
# Project Documentation

This document describes the architecture and usage of the vectorless library.

## Overview

Vectorless is a document indexing and retrieval library that uses tree-based navigation instead of vector embeddings.

### Key Features

- **Tree-Based Indexing** — Documents are organized as hierarchical trees
- **LLM Navigation** — Intelligent traversal using LLM to find relevant content
- **No Vector Database** — Eliminates infrastructure complexity

## Architecture

The crate is organized into several modules:

### Core Module

The core module provides fundamental types:
- `DocumentTree` — Arena-based tree structure
- `TreeNode` — A node in the document tree
- `NodeId` — Unique identifier for tree nodes

### Parser Module

The parser module handles document parsing:
- `MarkdownParser` — Parse Markdown files
- `PdfParser` — Parse PDF files (planned)
- `HtmlParser` — Parse HTML files (planned)