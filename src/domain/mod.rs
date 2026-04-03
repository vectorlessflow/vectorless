// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Domain layer - pure data structures with zero business logic.
//!
//! This module contains the core domain types that represent document trees.
//! These types have no dependencies on indexing or retrieval logic.
//!
//! # Types
//!
//! - [`VectorlessNode`] - A node in the document tree
//! - [`VectorlessTree`] - Arena-based tree structure
//! - [`NodeId`] - Unique identifier for tree nodes
//! - [`TocView`] - Table of Contents generator
//! - [`Error`] - Domain error types

mod error;
mod node;
mod token;
mod toc;
mod tree;

pub use error::{Error, Result};
pub use node::{NodeId, VectorlessNode};
pub use token::{estimate_tokens, estimate_tokens_fast, estimate_tokens_batch};
pub use toc::{TocConfig, TocEntry, TocNode, TocView};
pub use tree::{DocumentStructure, StructureNode, VectorlessTree};

// Backward compatibility aliases
#[doc(hidden)]
pub type TreeNode = VectorlessNode;

#[doc(hidden)]
pub type DocumentTree = VectorlessTree;
