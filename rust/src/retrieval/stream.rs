// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Streaming retrieval events.
//!
//! When `RetrieveOptions::streaming` is enabled, retrieval emits
//! [`RetrieveEvent`]s incrementally as the pipeline progresses through
//! its stages (Analyze → Plan → Search → Evaluate).
//!
//! # Example
//!
//! ```rust,ignore
//! let options = RetrieveOptions::new().with_streaming(true);
//! let rx = client.query_stream(&tree, "query", &options).await?;
//!
//! while let Some(event) = rx.recv().await {
//!     match event {
//!         RetrieveEvent::Started { query, .. } => println!("Started: {query}"),
//!         RetrieveEvent::StageCompleted { stage, .. } => println!("Done: {stage}"),
//!         RetrieveEvent::Completed { response } => {
//!             println!("Confidence: {}", response.confidence);
//!             break;
//!         }
//!         RetrieveEvent::Error { message } => {
//!             eprintln!("Error: {message}");
//!             break;
//!         }
//!         _ => {}
//!     }
//! }
//! ```

use tokio::sync::mpsc;

use super::types::{RetrieveResponse, SufficiencyLevel};

/// Events emitted during streaming retrieval.
///
/// Each event represents a meaningful milestone in the retrieval pipeline.
/// The stream always terminates with either [`Completed`](RetrieveEvent::Completed)
/// or [`Error`](RetrieveEvent::Error).
#[derive(Debug, Clone)]
pub enum RetrieveEvent {
    /// Retrieval pipeline started.
    Started {
        /// The query string.
        query: String,
        /// Planned retrieval strategy name.
        strategy: String,
    },

    /// A pipeline stage completed.
    StageCompleted {
        /// Stage name (analyze, plan, search, evaluate).
        stage: String,
        /// Time spent in this stage (ms).
        elapsed_ms: u64,
    },

    /// A node was visited during tree traversal.
    NodeVisited {
        /// Node ID.
        node_id: String,
        /// Node title.
        title: String,
        /// Relevance score (0.0 - 1.0).
        score: f32,
    },

    /// Relevant content was found.
    ContentFound {
        /// Node ID.
        node_id: String,
        /// Node title.
        title: String,
        /// Short preview of the content.
        preview: String,
        /// Relevance score.
        score: f32,
    },

    /// Pipeline is backtracking to an earlier stage.
    Backtracking {
        /// Stage backtracking from.
        from: String,
        /// Stage backtracking to.
        to: String,
        /// Reason for backtracking.
        reason: String,
    },

    /// Sufficiency check result.
    SufficiencyCheck {
        /// Sufficiency level.
        level: SufficiencyLevel,
        /// Total tokens collected so far.
        tokens: usize,
    },

    /// Retrieval completed successfully with final results.
    Completed {
        /// The full retrieval response.
        response: RetrieveResponse,
    },

    /// An error occurred during retrieval.
    Error {
        /// Error message.
        message: String,
    },
}

/// Sender half for streaming retrieval events.
pub(crate) type RetrieveEventSender = mpsc::Sender<RetrieveEvent>;

/// Receiver half for streaming retrieval events.
pub type RetrieveEventReceiver = mpsc::Receiver<RetrieveEvent>;

/// Create a bounded channel for streaming retrieval events.
///
/// The bound defaults to 64 events. The sender will apply backpressure
/// when the receiver cannot keep up, preventing unbounded memory growth.
pub(crate) fn channel(bound: usize) -> (RetrieveEventSender, RetrieveEventReceiver) {
    mpsc::channel(bound)
}

/// Default channel bound for streaming events.
pub const DEFAULT_STREAM_BOUND: usize = 64;
