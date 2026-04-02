// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Search algorithm trait and common types.

use async_trait::async_trait;

use crate::core::VectorlessTree;
use super::super::types::{NavigationStep, SearchPath};
use super::super::RetrievalContext;

/// Result of a search operation.
#[derive(Debug, Clone)]
pub struct SearchResult {
    /// Paths found during search.
    pub paths: Vec<SearchPath>,
    /// Navigation trace.
    pub trace: Vec<NavigationStep>,
    /// Number of nodes visited.
    pub nodes_visited: usize,
    /// Number of iterations performed.
    pub iterations: usize,
}

impl Default for SearchResult {
    fn default() -> Self {
        Self {
            paths: Vec::new(),
            trace: Vec::new(),
            nodes_visited: 0,
            iterations: 0,
        }
    }
}

/// Configuration for search algorithms.
#[derive(Debug, Clone)]
pub struct SearchConfig {
    /// Maximum number of results to return.
    pub top_k: usize,
    /// Beam width for multi-path search.
    pub beam_width: usize,
    /// Maximum iterations.
    pub max_iterations: usize,
    /// Minimum score to include a path.
    pub min_score: f32,
    /// Whether to include leaf nodes only.
    pub leaf_only: bool,
}

impl Default for SearchConfig {
    fn default() -> Self {
        Self {
            top_k: 5,
            beam_width: 3,
            max_iterations: 10,
            min_score: 0.1,
            leaf_only: false,
        }
    }
}

/// Trait for tree search algorithms.
///
/// Implementations provide different strategies for exploring
/// the document tree to find relevant content.
#[async_trait]
pub trait SearchTree: Send + Sync {
    /// Search the tree for relevant nodes.
    ///
    /// # Arguments
    ///
    /// * `tree` - The document tree to search
    /// * `context` - Retrieval context with query information
    /// * `config` - Search configuration
    ///
    /// # Returns
    ///
    /// A `SearchResult` with paths and trace information.
    async fn search(
        &self,
        tree: &VectorlessTree,
        context: &RetrievalContext,
        config: &SearchConfig,
    ) -> SearchResult;

    /// Get the name of this search algorithm.
    fn name(&self) -> &str;
}
