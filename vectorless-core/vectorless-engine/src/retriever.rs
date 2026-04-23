// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Document retrieval client.
//!
//! This module provides query and retrieval operations for document content,
//! dispatching through the retrieval layer to the agent-based system.

use tracing::info;

use super::types::QueryResult;
use vectorless_agent::{
    self, config::AgentConfig, config::DocContext, config::Scope, config::WorkspaceContext,
    events::EventEmitter as AgentEventEmitter,
};
use vectorless_document::{DocumentTree, NavigationIndex, ReasoningIndex};
use vectorless_error::Result;
use vectorless_events::{EventEmitter, QueryEvent};
use vectorless_llm::LlmClient;
use vectorless_retrieval::{dispatcher, postprocessor};

/// Document retrieval client.
///
/// Delegates to the agent-based retrieval system.
pub(crate) struct RetrieverClient {
    /// LLM client for agent navigation decisions.
    llm: LlmClient,

    /// Agent configuration.
    config: AgentConfig,

    /// Event emitter.
    events: EventEmitter,
}

impl RetrieverClient {
    /// Create a new retriever client with an LLM client.
    pub fn new(llm: LlmClient) -> Self {
        Self {
            llm,
            config: AgentConfig::default(),
            events: EventEmitter::new(),
        }
    }

    /// Create with event emitter.
    pub fn with_events(mut self, events: EventEmitter) -> Self {
        self.events = events;
        self
    }

    /// Set custom agent configuration.
    pub fn with_config(mut self, config: AgentConfig) -> Self {
        self.config = config;
        self
    }

    /// Get a reference to the agent configuration.
    pub fn config(&self) -> &AgentConfig {
        &self.config
    }

    /// Get a reference to the LLM client.
    pub fn llm(&self) -> &LlmClient {
        &self.llm
    }

    /// Query documents through the agent-based retrieval system.
    ///
    /// - `skip_analysis = true` → `Scope::Specified` (user-specified docs, skip Orchestrator analysis)
    /// - `skip_analysis = false` → `Scope::Workspace` (full Orchestrator analysis flow)
    #[tracing::instrument(skip_all, fields(question = %question, docs = documents.len()))]
    pub async fn query(
        &self,
        documents: &[(DocumentTree, NavigationIndex, ReasoningIndex, String)],
        question: &str,
        skip_analysis: bool,
    ) -> Result<QueryResult> {
        self.events.emit_query(QueryEvent::Started {
            query: question.to_string(),
        });

        info!(
            docs = documents.len(),
            skip_analysis, "Querying: {:?}", question
        );

        let doc_contexts: Vec<DocContext> = documents
            .iter()
            .map(|(tree, nav, ridx, id)| DocContext {
                tree,
                nav_index: nav,
                reasoning_index: ridx,
                doc_name: id.as_str(),
            })
            .collect();

        let scope = if skip_analysis {
            Scope::Specified(doc_contexts)
        } else {
            Scope::Workspace(WorkspaceContext::new(doc_contexts))
        };

        let emitter = AgentEventEmitter::noop();
        let output =
            dispatcher::dispatch(question, scope, &self.config, &self.llm, &emitter).await?;

        let fallback_id = documents
            .first()
            .map(|(_, _, _, id)| id.as_str())
            .unwrap_or("");
        let items = postprocessor::to_results(&output, fallback_id);
        let result = QueryResult::new_with_items(items);

        self.events.emit_query(QueryEvent::Complete {
            total_results: result.len(),
            confidence: result.single().map(|i| i.confidence).unwrap_or(0.0),
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
            RetrieverClient::new(LlmClient::new(vectorless_llm::config::LlmConfig::default()));
    }
}
