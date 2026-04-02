// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Core module containing fundamental types and traits.
//!
//! This module provides the building blocks for document trees:
//! - [`VectorlessNode`] - A node in the document tree
//! - [`VectorlessTree`] - Arena-based tree structure
//! - [`NodeId`] - Unique identifier for tree nodes
//! - [`StructureNode`] - JSON export format for tree nodes
//! - [`DocumentStructure`] - JSON export format for document structure
//!
//! ## Index Pipeline
//!
//! The index module provides a modular document indexing pipeline:
//! - [`PipelineExecutor`] - Main entry point for indexing
//! - [`IndexStage`] - Stage interface for pipeline steps
//! - [`IndexOptions`] - Configuration for indexing operations
//!
//! ## Retrieval System
//!
//! The retriever module provides a hybrid retrieval architecture:
//! - [`AdaptiveRetriever`] - Main entry point for retrieval
//! - [`Retriever`] trait - Core retrieval interface
//! - [`RetrieveOptions`] - Configuration for retrieval operations
//! - [`RetrieveResponse`] - Results from retrieval operations

mod error;
mod node;
mod traits;
mod tree;
mod toc;

pub mod index;
pub mod retriever;

pub use error::{Error, Result};
pub use node::{NodeId, VectorlessNode};
pub use tree::{DocumentStructure, VectorlessTree, StructureNode};
pub use traits::*;
pub use toc::{TocView, TocNode, TocEntry, TocConfig};

// Re-export index types for convenience
pub use index::{
    IndexContext, IndexInput, IndexOptions, IndexResult, IndexStage,
    PipelineExecutor, IndexMetrics, SummaryStrategy,
    ChangeDetector, ChangeSet, PartialUpdater,
};

// Re-export retriever types for convenience
pub use retriever::{
    AdaptiveRetriever, Retriever, RetrieverError, RetrieverResult,
    RetrieveOptions, RetrieveResponse, RetrievalResult, RetrievalContext,
    QueryComplexity, StrategyPreference, SufficiencyLevel,
};

// Backward compatibility aliases
#[doc(hidden)]
pub type TreeNode = VectorlessNode;
#[doc(hidden)]
pub type DocumentTree = VectorlessTree;
