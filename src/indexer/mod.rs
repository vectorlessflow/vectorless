// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Document indexing module.
//!
//! This module provides functionality to build and maintain document indices:
//! - **Tree Building** — Convert raw nodes to hierarchical trees
//! - **Thinning** — Merge small nodes with parents
//! - **Merging** — Combine adjacent small nodes
//! - **Incremental** — Update indices when documents change
//!
//! # Example
//!
//! ```rust,no_run
//! use vectorless::indexer::TreeBuilder;
//! use vectorless::document::RawNode;
//!
//! let raw_nodes = vec![
//!     RawNode { level: 1, title: "Section 1".into(), ..Default::default() },
//!     RawNode { level: 2, title: "Subsection".into(), ..Default::default() },
//! ];
//!
//! let tree = TreeBuilder::new()
//!     .with_root_title("My Document")
//!     .build(raw_nodes);
//! ```

mod incremental;
mod merger;
mod thinner;
mod tree_builder;

// Re-export from crate::config
pub use crate::config::IndexerConfig;

// Re-export main types
pub use incremental::{IncrementalIndexer, DiffResult, diff_trees};
pub use merger::{merge_adjacent_small_nodes, merge_children_into_parent};
pub use thinner::{ThinningConfig, ThinningConfigBuilder, calculate_total_tokens, thin_raw_nodes, thin_tree, subtree_token_count};
pub use tree_builder::TreeBuilder;
