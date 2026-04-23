// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Configuration for document graph building and retrieval.

use serde::{Deserialize, Serialize};

/// Configuration for building the document graph.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentGraphConfig {
    /// Whether graph building is enabled.
    pub enabled: bool,
    /// Minimum Jaccard similarity for creating an edge.
    pub min_keyword_jaccard: f32,
    /// Minimum shared keywords to create an edge.
    pub min_shared_keywords: usize,
    /// Maximum top keywords per document node.
    pub max_keywords_per_doc: usize,
    /// Maximum edges per document node.
    pub max_edges_per_node: usize,
    /// Boost factor applied to graph-connected documents during retrieval.
    pub retrieval_boost_factor: f32,
}

impl Default for DocumentGraphConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            min_keyword_jaccard: 0.1,
            min_shared_keywords: 2,
            max_keywords_per_doc: 50,
            max_edges_per_node: 20,
            retrieval_boost_factor: 0.15,
        }
    }
}

impl DocumentGraphConfig {
    /// Create a new config with defaults.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a disabled config.
    pub fn disabled() -> Self {
        Self {
            enabled: false,
            ..Self::default()
        }
    }
}
