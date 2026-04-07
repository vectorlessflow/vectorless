// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Incremental indexing support.
//!
//! This module provides functionality to incrementally update
//! an existing document index when the source document changes.
//!
//! # Features
//!
//! - **Fine-grained change detection**: Uses subtree fingerprints to identify
//!   exactly which nodes changed
//! - **Processing version tracking**: Automatically reprocesses when algorithm
//!   versions change
//! - **Partial updates**: Only reprocess changed nodes

mod detector;
mod updater;

pub use detector::{
    ChangeDetector, ChangeDetectorState, ChangeSet, ChangeType, DocumentChangeInfo, NodeChange,
    compute_all_node_fingerprints, compute_tree_fingerprint,
};
pub use updater::PartialUpdater;
