// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! # vectorless
//!
//! Hierarchical, Reasoning-Native Document Intelligence Engine.
//!
//! A document indexing and retrieval library that uses tree-based navigation
//! instead of vector embeddings for RAG applications.
//!
//! ## Features
//!
//! - **Tree-Based Indexing** — Documents organized as hierarchical trees
//! - **LLM Navigation** — Intelligent traversal using LLM to find relevant content
//! - **No Vector Database** — Eliminates infrastructure complexity
//! - **Multiple Formats** — Support for Markdown, PDF, HTML, and more
//!
//! ## Quick Start
//!
//! ```rust,no_run
//! use vectorless::core::{DocumentTree, TreeNode};
//!
//! // Create a document tree
//! let mut tree = DocumentTree::new("Root", "Root content");
//!
//! // Add children
//! let root = tree.root();
//! let child = tree.add_child(root, "Section 1", "Content for section 1");
//!
//! // Navigate the tree
//! for node_id in tree.children(root) {
//!     if let Some(node) = tree.get(node_id) {
//!         println!("Title: {}", node.title);
//!     }
//! }
//! ```
//!
//! ## Architecture
//!
//! The crate is organized into the following modules:
//!
//! - [`core`] — Core types: TreeNode, DocumentTree, NodeId
//! - [`document`] — Document parsing: Markdown, PDF, HTML
//! - [`indexer`] — Index building: tree construction, thinning, merging
//! - [`summarizer`] — Summary generation
//! - [`retriever`] — Retrieval strategies
//! - [`ranking`] — Result ranking
//! - [`storage`] — Persistence and caching
//! - [`client`] — High-level API

pub mod core;
pub mod config;
pub mod document;
pub mod summarizer;
pub mod indexer;

// Re-exports for convenience
pub use core::{DocumentTree, NodeId, TreeNode, Error, Result};
pub use config::{Config, ConfigLoader, ConfigError, SummaryConfig};
pub use document::{DocumentParser, DocumentFormat, MarkdownParser, RawNode, ParseResult};
pub use summarizer::{summarize, LlmError};
pub use indexer::TreeBuilder;
