// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Event types for client operations.
//!
//! Provides enums for indexing and workspace events
//! that can be observed via [`EventEmitter`](super::EventEmitter).

use vectorless_document::DocumentFormat;

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
