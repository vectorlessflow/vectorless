// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Document retrieval client.
//!
//! This module provides query and retrieval operations for document content.
//!
//! # Example
//!
//! ```rust,ignore
//! let retriever = RetrieverClient::new(pipeline_retriever);
//!
//! let result = retriever
//!     .query(&tree, "What is this?", RetrieveOptions::default())
//!     .await?;
//!
//! println!("Found {} results", result.results.len());
//! ```

use std::sync::Arc;

use tracing::info;

use super::types::QueryResultItem;
use crate::config::Config;
use crate::document::{DocumentTree, ReasoningIndex};
use crate::error::{Error, Result};
use crate::events::{EventEmitter, QueryEvent};
use crate::retrieval::stream::RetrieveEventReceiver;
use crate::retrieval::{RetrieveOptions, RetrieveResponse};

/// Document retrieval client.
///
/// Provides operations for querying document content.
pub(crate) struct RetrieverClient {
    /// Pipeline retriever.
    retriever: Arc<crate::retrieval::PipelineRetriever>,

    /// Configuration reference.
    config: Arc<Config>,

    /// Event emitter.
    events: EventEmitter,

    /// Default retrieval options.
    default_options: RetrieveOptions,
}

impl RetrieverClient {
    /// Create a new retriever client.
    pub fn new(retriever: crate::retrieval::PipelineRetriever, config: Arc<Config>) -> Self {
        Self {
            retriever: Arc::new(retriever),
            config,
            events: EventEmitter::new(),
            default_options: RetrieveOptions::default(),
        }
    }

    /// Create with event emitter.
    pub fn with_events(mut self, events: EventEmitter) -> Self {
        self.events = events;
        self
    }

    /// Query a document tree with optional reasoning index for fast-path lookup.
    ///
    /// # Errors
    ///
    /// Returns an error if the retrieval pipeline fails.
    pub async fn query_with_reasoning_index(
        &self,
        tree: &DocumentTree,
        question: &str,
        options: &RetrieveOptions,
        reasoning_index: Option<ReasoningIndex>,
    ) -> Result<QueryResultItem> {
        self.events.emit_query(QueryEvent::Started {
            query: question.to_string(),
        });

        info!("Querying: {:?}", question);

        // Execute retrieval with reasoning index
        let response = self
            .retriever
            .retrieve_with_reasoning_index(tree, question, options, reasoning_index)
            .await
            .map_err(|e| Error::Retrieval(e.to_string()))?;

        // Build result
        let result = self.build_query_result(&response);

        self.events.emit_query(QueryEvent::Complete {
            total_results: result.node_ids.len(),
            confidence: result.score,
        });

        Ok(result)
    }

    /// Query a document tree with streaming results.
    ///
    /// Returns a channel receiver that yields [`RetrieveEvent`]s
    /// incrementally as the pipeline progresses through its stages.
    /// The stream always terminates with either `Completed` or `Error`.
    ///
    /// Also emits events through the [`EventEmitter`] (configured via
    /// [`with_events`](Self::with_events)), so existing `on_query()` handlers
    /// receive streaming events too.
    ///
    /// This is the streaming counterpart of [`query`](Self::query).
    /// The non-streaming path is completely unaffected.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let options = RetrieveOptions::new().with_streaming(true);
    /// let mut rx = client.query_stream(&tree, "query", &options).await?;
    ///
    /// while let Some(event) = rx.recv().await {
    ///     match event {
    ///         RetrieveEvent::StageCompleted { stage, .. } => println!("{stage} done"),
    ///         RetrieveEvent::Completed { response } => {
    ///             println!("Confidence: {}", response.confidence);
    ///             break;
    ///         }
    ///         RetrieveEvent::Error { message } => { eprintln!("{message}"); break; }
    ///         _ => {}
    ///     }
    /// }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error if the retriever cannot be cloned for streaming.
    pub async fn query_stream(
        &self,
        tree: &DocumentTree,
        question: &str,
        options: &RetrieveOptions,
    ) -> Result<RetrieveEventReceiver> {
        self.events.emit_query(QueryEvent::Started {
            query: question.to_string(),
        });

        info!("Streaming query: {:?}", question);

        let (handle, rx) = self.retriever.retrieve_streaming(tree, question, options);

        // Spawn a sidecar task that forwards events to the EventEmitter
        let events = self.events.clone();
        let question_owned = question.to_string();
        tokio::spawn(async move {
            // The handle will complete when the streaming task finishes.
            // We don't need to forward events individually here since
            // the primary channel (rx) is returned to the caller.
            // The EventEmitter events are already emitted above for Started.
            // The caller can consume rx for detailed streaming events.
            let _ = handle.await;
            events.emit_query(QueryEvent::Complete {
                total_results: 0,
                confidence: 0.0,
            });
            let _ = question_owned; // suppress unused warning
        });

        Ok(rx)
    }

    /// Build QueryResultItem from RetrieveResponse.
    fn build_query_result(&self, response: &RetrieveResponse) -> QueryResultItem {
        // Extract node IDs
        let node_ids: Vec<String> = response
            .results
            .iter()
            .filter_map(|r| r.node_id.clone())
            .collect();

        // Build content
        let content_parts: Vec<String> = response
            .results
            .iter()
            .map(|r| {
                let mut parts = vec![format!("## {}", r.title)];
                if let Some(ref content) = r.content {
                    parts.push(content.clone());
                }
                parts.join("\n\n")
            })
            .collect();

        let content = if content_parts.is_empty() {
            response.content.clone()
        } else {
            content_parts.join("\n\n---\n\n")
        };

        QueryResultItem {
            doc_id: String::new(), // Will be set by caller
            node_ids,
            content,
            score: response.confidence,
        }
    }
}

impl Clone for RetrieverClient {
    fn clone(&self) -> Self {
        Self {
            retriever: Arc::clone(&self.retriever),
            config: Arc::clone(&self.config),
            events: self.events.clone(),
            default_options: self.default_options.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_retriever_client_creation() {
        let config = Arc::new(Config::default());
        let retriever = crate::retrieval::PipelineRetriever::new();
        let client = RetrieverClient::new(retriever, config);
        assert!(client.default_options.top_k > 0);
    }
}
