// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Document graph module — workspace-level cross-document relationship graph.
//!
//! This module provides:
//! - [`DocumentGraph`] — the graph data structure connecting documents by shared concepts
//! - [`DocumentGraphBuilder`] — constructs the graph from document keyword profiles
//! - [`DocumentGraphConfig`] — configuration for graph building and retrieval boosting
//!
//! The document graph is a workspace-scoped, weighted graph built from each document's
//! [`ReasoningIndex`](crate::document::ReasoningIndex) keyword data. It enables
//! graph-aware retrieval ranking where connected documents receive a relevance boost.
//!
//! # Data Flow
//!
//! ```text
//! Document Indexing → ReasoningIndex (topic_paths)
//!                          ↓
//!              DocumentGraphBuilder::add_document()
//!                          ↓
//!                   DocumentGraph
//!                          ↓
//!                  Workspace::set_graph()
//!                          ↓
//!                 Engine::query() loads graph
//!                          ↓
//!           CrossDocumentStrategy (graph boosting)
//! ```

mod builder;
mod config;
mod types;

// Re-export public API
pub use builder::DocumentGraphBuilder;
pub use config::DocumentGraphConfig;
pub use types::{DocumentGraph, DocumentGraphNode, EdgeEvidence, GraphEdge, WeightedKeyword};
