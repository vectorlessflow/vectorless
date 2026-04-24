// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Document types - pure data structures for document tree representation.
//!
//! This module contains the core types that represent hierarchical documents.
//! These types have no dependencies on indexing or retrieval logic.
//!
//! # Types
//!
//! - [`TreeNode`] - A node in the document tree
//! - [`DocumentTree`] - Arena-based tree structure
//! - [`NodeId`] - Unique identifier for tree nodes
//! - [`TocView`] - Table of Contents generator
//! - [`StructureNode`] - JSON export structure
//! - [`NodeReference`] - In-document reference (e.g., "see Appendix G")
//! - [`RefType`] - Type of reference (Section, Appendix, Table, etc.)

mod format;
mod navigation;
mod node;
mod reasoning;
mod reference;
mod serde_helpers;
mod structure;
mod toc;
mod tree;
pub mod understanding;

// New: Agent acceleration types
mod chain;
mod evidence;
mod overlap;
mod query_route;

pub use format::DocumentFormat;
pub use navigation::{ChildRoute, DocCard, NavEntry, NavigationIndex, SectionCard};
pub use node::{NodeId, TreeNode};
pub use reasoning::{
    ReasoningIndex, ReasoningIndexBuilder, ReasoningIndexConfig, SectionSummary, SummaryShortcut,
    TopicEntry,
};
pub use reference::{NodeReference, ReferenceExtractor};
pub use structure::{DocumentStructure, StructureNode};
pub use toc::{TocConfig, TocEntry, TocNode, TocView};
pub use tree::{DocumentTree, RetrievalIndex};
pub use understanding::{Concept, Document, DocumentInfo, IngestInput};

// Re-export agent acceleration types
pub use chain::{ChainIndex, ChainType, ReasoningChain};
pub use evidence::{EvidenceScore, EvidenceScoreMap};
pub use overlap::{ContentOverlapMap, OverlapEntry, OverlapType};
pub use query_route::{ConceptRoute, QueryRoutingTable, RouteTarget};
