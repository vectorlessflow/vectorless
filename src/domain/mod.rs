// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Domain layer - pure data structures with zero business logic.
//!
//! This module contains the core domain types that represent document trees.
//! These types have no dependencies on indexing or retrieval logic.
//!
//! # Types
//!
//! - [`TreeNode`] - A node in the document tree
//! - [`DocumentTree`] - Arena-based tree structure
//! - [`NodeId`] - Unique identifier for tree nodes
//! - [`TocView`] - Table of Contents generator
//! - [`Error`] - Domain error types

mod error;
mod node;
mod toc;
mod token;
mod tree;

pub use error::{Error, Result};
pub use node::{NodeId, TreeNode};
pub use toc::{TocConfig, TocEntry, TocNode, TocView};
pub use token::{estimate_tokens, estimate_tokens_batch, estimate_tokens_fast};
pub use tree::{DocumentStructure, DocumentTree, RetrievalIndex, StructureNode};
