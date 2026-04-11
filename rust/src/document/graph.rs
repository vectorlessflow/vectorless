// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Re-export all graph types from the standalone `graph` module.
//!
//! This shim preserves backward compatibility for code importing
//! from `crate::document::DocumentGraph`.

pub use crate::graph::{
    DocumentGraph, DocumentGraphConfig, DocumentGraphNode, EdgeEvidence, GraphEdge, GraphMetadata,
    KeywordDocEntry, SharedKeyword, WeightedKeyword,
};
