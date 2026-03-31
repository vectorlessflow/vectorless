// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Core module containing fundamental types and traits.
//!
//! This module provides the building blocks for document trees:
//! - [`TreeNode`] - A node in the document tree
//! - [`DocumentTree`] - Arena-based tree structure
//! - [`NodeId`] - Unique identifier for tree nodes
//! - [`StructureNode`] - JSON export format for tree nodes
//! - [`DocumentStructure`] - JSON export format for document structure

mod error;
mod node;
mod traits;
mod tree;
mod types;

pub use error::{Error, Result};
pub use node::{NodeId, TreeNode};
pub use tree::{DocumentStructure, DocumentTree, StructureNode};
pub use traits::*;
