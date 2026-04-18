// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Document retrieval client.
//!
//! This module provides query and retrieval operations for document content,
//! using the agent-based retrieval system.

use tracing::info;

use super::types::QueryResultItem;
use crate::document::{DocumentTree, NavigationIndex, ReasoningIndex};
use crate::error::{Error, Result};
use crate::events::{EventEmitter, QueryEvent};
use crate::llm::LlmClient;
use crate::retrieval::agent;

/// Document retrieval client.
///
/// Delegates to the agent-based retrieval system.
pub(crate) struct RetrieverClient {
    /// LLM client for agent navigation decisions.
    llm: LlmClient,

    /// Agent configuration.
    config: agent::Config,

    /// Event emitter.
    events: EventEmitter,
}

impl RetrieverClient {
    /// Create a new retriever client with an LLM client.
    pub fn new(llm: LlmClient) -> Self {
        Self {
            llm,
            config: agent::Config::default(),
            events: EventEmitter::new(),
        }
    }

    /// Create with event emitter.
    pub fn with_events(mut self, events: EventEmitter) -> Self {
        self.events = events;
        self
    }

    /// Set custom agent configuration.
    pub fn with_config(mut self, config: agent::Config) -> Self {
        self.config = config;
        self
    }

    /// Query a single document tree.
    #[tracing::instrument(skip_all, fields(question = %question))]
    pub async fn query_single(
        &self,
        tree: &DocumentTree,
        nav_index: &NavigationIndex,
        reasoning_index: &ReasoningIndex,
        question: &str,
        doc_name: &str,
    ) -> Result<QueryResultItem> {
        self.events.emit_query(QueryEvent::Started {
            query: question.to_string(),
        });

        info!("Querying: {:?}", question);

        let doc_ctx = agent::DocContext {
            tree,
            nav_index,
            reasoning_index,
            doc_name,
        };

        let scope = agent::Scope::Single(doc_ctx);
        let output = agent::retrieve(question, scope, &self.config, &self.llm)
            .await
            .map_err(|e| Error::Retrieval(e.to_string()))?;

        let result = self.build_query_result(&output);

        self.events.emit_query(QueryEvent::Complete {
            total_results: result.node_ids.len(),
            confidence: result.score,
        });

        Ok(result)
    }

    /// Query multiple documents using the Orchestrator.
    #[tracing::instrument(skip_all, fields(question = %question))]
    pub async fn query_multi(
        &self,
        documents: &[(DocumentTree, NavigationIndex, ReasoningIndex, String)],
        question: &str,
    ) -> Result<QueryResultItem> {
        self.events.emit_query(QueryEvent::Started {
            query: question.to_string(),
        });

        info!(docs = documents.len(), "Multi-doc querying: {:?}", question);

        let doc_contexts: Vec<agent::DocContext> = documents
            .iter()
            .map(|(tree, nav, ridx, name)| agent::DocContext {
                tree,
                nav_index: nav,
                reasoning_index: ridx,
                doc_name: name.as_str(),
            })
            .collect();

        let ws = agent::WorkspaceContext::new(doc_contexts);
        let scope = agent::Scope::Workspace(ws);

        let output = agent::retrieve(question, scope, &self.config, &self.llm)
            .await
            .map_err(|e| Error::Retrieval(e.to_string()))?;

        let result = self.build_multi_query_result(&output);

        self.events.emit_query(QueryEvent::Complete {
            total_results: result.node_ids.len(),
            confidence: result.score,
        });

        Ok(result)
    }

    /// Build QueryResultItem from agent Output.
    fn build_query_result(&self, output: &agent::Output) -> QueryResultItem {
        let node_ids: Vec<String> = output
            .evidence
            .iter()
            .map(|e| e.source_path.clone())
            .collect();

        let content = if output.answer.is_empty() {
            output
                .evidence
                .iter()
                .map(|e| format!("## {}\n{}", e.node_title, e.content))
                .collect::<Vec<_>>()
                .join("\n\n---\n\n")
        } else {
            output.answer.clone()
        };

        // Confidence based on whether we found evidence
        let score = if output.evidence.is_empty() {
            0.0
        } else {
            0.8 // Agent-based retrieval is high confidence when it finds evidence
        };

        QueryResultItem {
            doc_id: String::new(), // Set by caller
            node_ids,
            content,
            score,
        }
    }

    /// Build QueryResultItem from multi-doc agent output.
    fn build_multi_query_result(&self, output: &agent::Output) -> QueryResultItem {
        let node_ids: Vec<String> = output
            .evidence
            .iter()
            .map(|e| e.source_path.clone())
            .collect();

        QueryResultItem {
            doc_id: String::new(),
            node_ids,
            content: output.answer.clone(),
            score: if output.evidence.is_empty() { 0.0 } else { 0.8 },
        }
    }
}

impl Clone for RetrieverClient {
    fn clone(&self) -> Self {
        Self {
            llm: self.llm.clone(),
            config: self.config.clone(),
            events: self.events.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_retriever_client_creation() {
        let _client = RetrieverClient::new(LlmClient::new(
            crate::llm::config::LlmConfig::default(),
        ));
    }
}
