// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! High-level client API for document compilation and retrieval.
//!
//! This module provides the main entry point for using vectorless:
//! - [`Engine`] — The main client for compiling and querying documents
//! - [`EngineBuilder`] — Builder pattern for client configuration
//! - [`CompileInput`] — Unified input for document compilation
//!
//! Retrieval (ask) is handled by the Python strategy layer.

mod builder;
mod compile_input;
mod engine;
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

pub use compile_input::CompileInput;

// ============================================================
// Result & Info Types
// ============================================================

pub use types::{CompileArtifact, CompileMode, CompileOptions, CompileOutput, FailedItem};

// ============================================================
// Parser Types (needed for CompileInput::from_content)
// ============================================================

pub use vectorless_document::DocumentFormat;

// ============================================================
// Re-exports from sub-crates (for downstream consumers)
// ============================================================

pub use vectorless_config::Config;
pub use vectorless_document::DocumentTree;
pub use vectorless_document::{Concept, DocumentInfo, IngestInput, RawNodeInput};
pub use vectorless_error::{Error, Result};
pub use vectorless_events::{CompileEvent, EventEmitter, WorkspaceEvent};
pub use vectorless_graph::{
    DocumentGraph, DocumentGraphNode, EdgeEvidence, GraphEdge, WeightedKeyword,
};
pub use vectorless_metrics::{LlmMetricsReport, MetricsReport, RetrievalMetricsReport};
