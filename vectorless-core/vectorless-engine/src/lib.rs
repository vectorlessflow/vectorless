// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! High-level client API for document indexing and retrieval.
//!
//! This module provides the main entry point for using vectorless:
//! - [`Engine`] — The main client for indexing and querying documents
//! - [`EngineBuilder`] — Builder pattern for client configuration
//! - [`IndexContext`] — Unified input for document indexing
//!
//! Retrieval (ask) is handled by the Python strategy layer.

mod builder;
mod engine;
mod index_context;
mod indexed_document;
mod indexer;
mod types;
mod workspace;

// ============================================================
// Main Types
// ============================================================

pub use builder::{BuildError, EngineBuilder};
pub use engine::Engine;

// ============================================================
// Context Types
// ============================================================

pub use index_context::IndexContext;

// ============================================================
// Result & Info Types
// ============================================================

pub use types::{
    Confidence, EvidenceItem, FailedItem, IndexItem, IndexMode, IndexOptions, IndexResult,
    QueryMetrics, QueryResult, QueryResultItem,
};

// ============================================================
// Parser Types (needed for IndexContext::from_content)
// ============================================================

pub use vectorless_document::DocumentFormat;

// ============================================================
// Re-exports from sub-crates (for downstream consumers)
// ============================================================

pub use vectorless_config::Config;
pub use vectorless_document::DocumentTree;
pub use vectorless_document::{
    Answer, Concept, DocumentInfo, Evidence, IngestInput, ReasoningTrace, TraceStep,
};
pub use vectorless_error::{Error, Result};
pub use vectorless_events::{EventEmitter, IndexEvent, QueryEvent, WorkspaceEvent};
pub use vectorless_graph::{
    DocumentGraph, DocumentGraphNode, EdgeEvidence, GraphEdge, WeightedKeyword,
};
pub use vectorless_metrics::{LlmMetricsReport, MetricsReport, RetrievalMetricsReport};
