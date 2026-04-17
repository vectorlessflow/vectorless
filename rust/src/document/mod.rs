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

mod graph;
mod node;
mod reasoning;
mod reference;
mod structure;
mod toc;
mod tree;

pub use node::{NodeId, TreeNode};
pub use reasoning::{
    HotNodeEntry, ReasoningIndex, ReasoningIndexBuilder, ReasoningIndexConfig, SectionSummary,
    SummaryShortcut, TopicEntry,
};
pub use reference::{NodeReference, RefType, ReferenceExtractor};
pub use structure::{DocumentStructure, StructureNode};
pub use toc::{TocConfig, TocEntry, TocNode, TocView};
pub use tree::{DocumentTree, RetrievalIndex};
