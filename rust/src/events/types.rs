// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Event types for client operations.
//!
//! Provides enums for indexing, query, and workspace events
//! that can be observed via [`EventEmitter`](super::EventEmitter).

use crate::index::parse::DocumentFormat;
use crate::retrieval::SufficiencyLevel;

/// Top-level event types for client operations.
#[derive(Debug, Clone)]
pub enum Event {
    /// Indexing events.
    Index(IndexEvent),

    /// Query events.
    Query(QueryEvent),

    /// Workspace events.
    Workspace(WorkspaceEvent),
}

/// Indexing operation events.
#[derive(Debug, Clone)]
pub enum IndexEvent {
    /// Started indexing a document.
    Started {
        /// File path being indexed.
        path: String,
    },

    /// Document format detected.
    FormatDetected {
        /// Detected format.
        format: DocumentFormat,
    },

    /// Parsing progress update.
    ParsingProgress {
        /// Percentage complete (0-100).
        percent: u8,
    },

    /// Document tree built.
    TreeBuilt {
        /// Number of nodes in the tree.
        node_count: usize,
    },

    /// Summary generation progress.
    SummaryProgress {
        /// Number of summaries completed.
        completed: usize,
        /// Total summaries to generate.
        total: usize,
    },

    /// Indexing completed successfully.
    Complete {
        /// Generated document ID.
        doc_id: String,
    },

    /// Error occurred during indexing.
    Error {
        /// Error message.
        message: String,
    },
}

/// Query operation events.
#[derive(Debug, Clone)]
pub enum QueryEvent {
    /// Search started.
    Started {
        /// The query string.
        query: String,
    },

    /// Node visited during search.
    NodeVisited {
        /// Node ID.
        node_id: String,
        /// Node title.
        title: String,
        /// Relevance score.
        score: f32,
    },

    /// Candidate result found.
    CandidateFound {
        /// Node ID.
        node_id: String,
        /// Relevance score.
        score: f32,
    },

    /// Sufficiency check result.
    SufficiencyCheck {
        /// Sufficiency level.
        level: SufficiencyLevel,
        /// Total tokens collected.
        tokens: usize,
    },

    /// Query completed.
    Complete {
        /// Total results found.
        total_results: usize,
        /// Overall confidence score.
        confidence: f32,
    },

    /// Error occurred during query.
    Error {
        /// Error message.
        message: String,
    },
}

/// Workspace operation events.
#[derive(Debug, Clone)]
pub enum WorkspaceEvent {
    /// Document saved to workspace.
    Saved {
        /// Document ID.
        doc_id: String,
    },

    /// Document loaded from workspace.
    Loaded {
        /// Document ID.
        doc_id: String,
        /// Whether it was a cache hit.
        cache_hit: bool,
    },

    /// Document removed from workspace.
    Removed {
        /// Document ID.
        doc_id: String,
    },

    /// Workspace cleared.
    Cleared {
        /// Number of documents removed.
        count: usize,
    },
}
