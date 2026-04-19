// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Document retrieval client.
//!
//! This module provides query and retrieval operations for document content,
//! dispatching through the retrieval layer to the agent-based system.

use tracing::info;

use crate::agent::{self, events::EventEmitter as AgentEventEmitter};
use crate::client::types::QueryResult;
use crate::document::{DocumentTree, NavigationIndex, ReasoningIndex};
use crate::error::Result;
use crate::events::{EventEmitter, QueryEvent};
use crate::llm::LlmClient;
use crate::retrieval::{dispatcher, postprocessor};

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

    /// Get a reference to the agent configuration.
    pub fn config(&self) -> &agent::Config {
        &self.config
    }

    /// Get a reference to the LLM client.
    pub fn llm(&self) -> &LlmClient {
        &self.llm
    }

    /// Query a single document tree.
    #[tracing::instrument(skip_all, fields(question = %question))]
    pub async fn query_single(
        &self,
        tree: &DocumentTree,
        nav_index: &NavigationIndex,
        reasoning_index: &ReasoningIndex,
        question: &str,
        doc_id: &str,
    ) -> Result<QueryResult> {
        self.events.emit_query(QueryEvent::Started {
            query: question.to_string(),
        });

        info!("Querying: {:?}", question);

        let doc_ctx = agent::DocContext {
            tree,
            nav_index,
            reasoning_index,
            doc_name: doc_id,
        };

        let scope = agent::Scope::Specified(vec![doc_ctx]);
        let emitter = AgentEventEmitter::noop();
        let output =
            dispatcher::dispatch(question, scope, &self.config, &self.llm, &emitter).await?;

        let items = postprocessor::to_results(&output, doc_id);
        let result = QueryResult::new_with_items(items);

        self.events.emit_query(QueryEvent::Complete {
            total_results: result.len(),
            confidence: result.single().map(|i| i.score).unwrap_or(0.0),
        });

        Ok(result)
    }

    /// Query multiple documents using the Orchestrator.
    #[tracing::instrument(skip_all, fields(question = %question))]
    pub async fn query_multi(
        &self,
        documents: &[(DocumentTree, NavigationIndex, ReasoningIndex, String)],
        question: &str,
    ) -> Result<QueryResult> {
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
        let emitter = AgentEventEmitter::noop();

        let output =
            dispatcher::dispatch(question, scope, &self.config, &self.llm, &emitter).await?;

        // Use first doc_id as fallback for evidence without doc_name
        let fallback_id = documents
            .first()
            .map(|(_, _, _, id)| id.as_str())
            .unwrap_or("");
        let items = postprocessor::to_results(&output, fallback_id);
        let result = QueryResult::new_with_items(items);

        self.events.emit_query(QueryEvent::Complete {
            total_results: result.len(),
            confidence: result.single().map(|i| i.score).unwrap_or(0.0),
        });

        Ok(result)
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
        let _client =
            RetrieverClient::new(LlmClient::new(crate::llm::config::LlmConfig::default()));
    }
}
