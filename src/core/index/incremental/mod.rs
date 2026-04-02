// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Incremental indexing support.
//!
//! This module provides functionality to incrementally update
//! an existing document index when the source document changes.

mod detector;
mod updater;

pub use detector::{ChangeDetector, ChangeSet, ChangeType};
pub use updater::PartialUpdater;
