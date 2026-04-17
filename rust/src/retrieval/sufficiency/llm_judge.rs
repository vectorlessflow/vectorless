// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! LLM-based sufficiency checker.
//!
//! Uses an LLM to judge whether collected content is sufficient.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use super::{SufficiencyChecker, SufficiencyLevel};
use crate::config::SufficiencyConfig;
use crate::llm::memo::{MemoKey, MemoOpType, MemoStore, MemoValue};
use crate::utils::fingerprint::Fingerprint;

/// LLM client trait for the judge.
#[async_trait]
pub trait LlmJudgeClient: Send + Sync {
    /// Generate a completion.
    async fn complete(&self, prompt: &str) -> Result<String, JudgeError>;
}

/// Error type for LLM judge.
#[derive(Debug, thiserror::Error)]
pub enum JudgeError {
    #[error("LLM request failed: {0}")]
    RequestFailed(String),
    #[error("Failed to parse response: {0}")]
    ParseError(String),
}

/// Response from LLM judge.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct JudgeResponse {
    /// Whether content is sufficient.
    sufficient: bool,
    /// Confidence level (0-1).
    confidence: f32,
    /// Optional reasoning.
    #[serde(default)]
    reasoning: Option<String>,
}

/// LLM-based sufficiency judge.
///
/// Uses an LLM to determine if the collected content
/// is sufficient to answer the query.
pub struct LlmJudge {
    client: Box<dyn LlmJudgeClient>,
    /// System prompt for the judge.
    system_prompt: String,
    /// Minimum confidence to consider sufficient.
    confidence_threshold: f32,
    /// Memo store for caching sufficiency judgments.
    memo_store: Option<MemoStore>,
}

impl LlmJudge {
    /// Create a new LLM judge.
    pub fn new(client: Box<dyn LlmJudgeClient>) -> Self {
        Self::with_config(client, &SufficiencyConfig::default())
    }

    /// Create a new LLM judge with configuration.
    pub fn with_config(client: Box<dyn LlmJudgeClient>, config: &SufficiencyConfig) -> Self {
        Self {
            client,
            system_prompt: Self::default_system_prompt(),
            confidence_threshold: config.confidence_threshold,
            memo_store: None,
        }
    }

    /// Add memo store for caching sufficiency judgments.
    ///
    /// When enabled, sufficiency check results are cached based on
    /// query+content fingerprints, avoiding redundant LLM calls.
    pub fn with_memo_store(mut self, store: MemoStore) -> Self {
        self.memo_store = Some(store);
        self
    }

    /// Set confidence threshold.
    pub fn with_confidence_threshold(mut self, threshold: f32) -> Self {
        self.confidence_threshold = threshold;
        self
    }

    fn default_system_prompt() -> String {
        r#"You are a content sufficiency judge. Your task is to determine if the provided content is sufficient to answer the given query.

Respond in JSON format:
{"sufficient": <true|false>, "confidence": <0.0-1.0>, "reasoning": "<brief explanation>"}

Guidelines:
- "sufficient" should be true only if the content directly addresses the query
- "confidence" should reflect how certain you are in your judgment
- Consider: completeness, relevance, and accuracy of the information

Be conservative - only mark as sufficient if you're confident the content answers the query."#
            .to_string()
    }

    fn build_prompt(&self, query: &str, content: &str) -> String {
        format!(
            "{}\n\nQuery: {}\n\nContent:\n{}\n\nIs this content sufficient to answer the query?",
            self.system_prompt, query, content
        )
    }

    fn parse_response(&self, response: &str) -> (SufficiencyLevel, f32) {
        // Try JSON parsing
        if let Ok(parsed) = serde_json::from_str::<JudgeResponse>(response) {
            let level = if parsed.sufficient && parsed.confidence >= self.confidence_threshold {
                SufficiencyLevel::Sufficient
            } else if parsed.confidence >= 0.5 {
                SufficiencyLevel::PartialSufficient
            } else {
                SufficiencyLevel::Insufficient
            };
            return (level, parsed.confidence);
        }

        // Fallback: keyword analysis
        let lower = response.to_lowercase();
        let sufficient_keywords = ["sufficient", "yes", "complete", "enough"];
        let insufficient_keywords = ["insufficient", "no", "incomplete", "not enough"];

        let sufficient_count = sufficient_keywords
            .iter()
            .filter(|k| lower.contains(*k))
            .count();
        let insufficient_count = insufficient_keywords
            .iter()
            .filter(|k| lower.contains(*k))
            .count();

        if sufficient_count > insufficient_count {
            (SufficiencyLevel::PartialSufficient, 0.6)
        } else {
            (SufficiencyLevel::Insufficient, 0.4)
        }
    }

    /// Check sufficiency asynchronously.
    pub async fn check_async(
        &self,
        query: &str,
        content: &str,
        _token_count: usize,
    ) -> SufficiencyLevel {
        // Check memo cache
        if let Some(ref store) = self.memo_store {
            let cache_key = self.build_cache_key(query, content);
            if let Some(cached) = store.get(&cache_key) {
                if let Some(level) = Self::deserialize_sufficiency(&cached) {
                    tracing::debug!("Memo cache hit for sufficiency check");
                    return level;
                }
            }
        }

        let prompt = self.build_prompt(query, content);

        let result = match self.client.complete(&prompt).await {
            Ok(response) => self.parse_response(&response).0,
            Err(_) => SufficiencyLevel::Insufficient,
        };

        // Cache the result
        if let Some(ref store) = self.memo_store {
            let cache_key = self.build_cache_key(query, content);
            let tokens = (prompt.len() / 4) as u64;
            store.put_with_tokens(
                cache_key,
                MemoValue::Text(format!("{:?}", result)),
                tokens,
            );
        }

        result
    }

    /// Build a cache key for sufficiency check.
    fn build_cache_key(&self, query: &str, content: &str) -> MemoKey {
        let mut input = String::with_capacity(query.len() + content.len() / 4);
        input.push_str(query);
        // Use only first 2000 chars of content for fingerprint to avoid
        // giant cache keys — content prefix captures topic identity.
        input.push_str(&content[..2000.min(content.len())]);
        let fp = Fingerprint::from_str(&input);
        MemoKey {
            op_type: MemoOpType::SufficiencyCheck,
            input_fp: fp,
            model_id: None,
            version: 1,
            context_fp: Fingerprint::zero(),
        }
    }

    /// Deserialize a SufficiencyLevel from a MemoValue.
    fn deserialize_sufficiency(value: &MemoValue) -> Option<SufficiencyLevel> {
        match value {
            MemoValue::Text(s) => match s.as_str() {
                "Sufficient" => Some(SufficiencyLevel::Sufficient),
                "PartialSufficient" => Some(SufficiencyLevel::PartialSufficient),
                "Insufficient" => Some(SufficiencyLevel::Insufficient),
                _ => None,
            },
            _ => None,
        }
    }
}

impl SufficiencyChecker for LlmJudge {
    fn check(&self, query: &str, content: &str, token_count: usize) -> SufficiencyLevel {
        // For synchronous usage, we use a simple heuristic
        // The async version should be preferred when possible

        // Quick content analysis
        if content.is_empty() {
            return SufficiencyLevel::Insufficient;
        }

        // Check for query terms in content
        let query_terms: Vec<&str> = query.split_whitespace().collect();
        let content_lower = content.to_lowercase();

        let matches: usize = query_terms
            .iter()
            .filter(|term| content_lower.contains(&term.to_lowercase()))
            .count();

        let coverage = if query_terms.is_empty() {
            0.0
        } else {
            matches as f32 / query_terms.len() as f32
        };

        if coverage > 0.8 && token_count > 500 {
            SufficiencyLevel::Sufficient
        } else if coverage > 0.5 {
            SufficiencyLevel::PartialSufficient
        } else {
            SufficiencyLevel::Insufficient
        }
    }

    fn name(&self) -> &'static str {
        "llm_judge"
    }
}

/// Adapter to use LlmClient as LlmJudgeClient.
#[async_trait]
impl LlmJudgeClient for crate::llm::LlmClient {
    async fn complete(&self, prompt: &str) -> Result<String, JudgeError> {
        self.complete("You are a content sufficiency judge.", prompt)
            .await
            .map_err(|e| JudgeError::RequestFailed(e.to_string()))
    }
}
