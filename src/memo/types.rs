// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Types for the memoization system.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::utils::fingerprint::Fingerprint;

/// Types of operations that can be memoized.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MemoOpType {
    /// Node summary generation.
    Summary,

    /// Pilot navigation decision.
    PilotDecision,

    /// Query analysis result.
    QueryAnalysis,

    /// Content extraction result.
    Extraction,

    /// Custom operation type.
    Custom(u8),
}

impl MemoOpType {
    /// Get a unique byte identifier for this operation type.
    pub fn as_byte(&self) -> u8 {
        match self {
            MemoOpType::Summary => 0,
            MemoOpType::PilotDecision => 1,
            MemoOpType::QueryAnalysis => 2,
            MemoOpType::Extraction => 3,
            MemoOpType::Custom(n) => 100 + n,
        }
    }
}

/// Key for memoization lookup.
///
/// Keys are content-addressed using fingerprints, ensuring that
/// cache hits only occur when the input is semantically identical.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MemoKey {
    /// Type of operation being memoized.
    pub op_type: MemoOpType,

    /// Fingerprint of the input content.
    pub input_fp: Fingerprint,

    /// Optional model identifier for cache invalidation when model changes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_id: Option<String>,

    /// Optional version for cache invalidation when algorithm changes.
    #[serde(default)]
    pub version: u32,

    /// Additional context fingerprint (e.g., query context for pilot decisions).
    #[serde(default, skip_serializing_if = "Fingerprint::is_zero")]
    pub context_fp: Fingerprint,
}

impl MemoKey {
    /// Create a key for summary generation.
    pub fn summary(content_fp: &Fingerprint) -> Self {
        Self {
            op_type: MemoOpType::Summary,
            input_fp: *content_fp,
            model_id: None,
            version: 1,
            context_fp: Fingerprint::zero(),
        }
    }

    /// Create a key for summary generation with model and version.
    pub fn summary_with_model(content_fp: &Fingerprint, model_id: &str, version: u32) -> Self {
        Self {
            op_type: MemoOpType::Summary,
            input_fp: *content_fp,
            model_id: Some(model_id.to_string()),
            version,
            context_fp: Fingerprint::zero(),
        }
    }

    /// Create a key for pilot decision.
    pub fn pilot_decision(context_fp: &Fingerprint, query_fp: &Fingerprint) -> Self {
        Self {
            op_type: MemoOpType::PilotDecision,
            input_fp: *query_fp,
            model_id: None,
            version: 1,
            context_fp: *context_fp,
        }
    }

    /// Create a key for query analysis.
    pub fn query_analysis(query_fp: &Fingerprint) -> Self {
        Self {
            op_type: MemoOpType::QueryAnalysis,
            input_fp: *query_fp,
            model_id: None,
            version: 1,
            context_fp: Fingerprint::zero(),
        }
    }

    /// Create a key for content extraction.
    pub fn extraction(content_fp: &Fingerprint) -> Self {
        Self {
            op_type: MemoOpType::Extraction,
            input_fp: *content_fp,
            model_id: None,
            version: 1,
            context_fp: Fingerprint::zero(),
        }
    }

    /// Set the model identifier.
    pub fn with_model(mut self, model_id: &str) -> Self {
        self.model_id = Some(model_id.to_string());
        self
    }

    /// Set the version.
    pub fn with_version(mut self, version: u32) -> Self {
        self.version = version;
        self
    }

    /// Set the context fingerprint.
    pub fn with_context(mut self, context_fp: &Fingerprint) -> Self {
        self.context_fp = *context_fp;
        self
    }

    /// Compute a fingerprint of this key for storage.
    pub fn fingerprint(&self) -> Fingerprint {
        use crate::utils::fingerprint::Fingerprinter;

        let mut fp = Fingerprinter::new();
        fp.write_u64(self.op_type.as_byte() as u64);
        fp.write_fingerprint(&self.input_fp);
        fp.write_option_str(self.model_id.as_deref());
        fp.write_u64(self.version as u64);
        if !self.context_fp.is_zero() {
            fp.write_fingerprint(&self.context_fp);
        }
        fp.into_fingerprint()
    }
}

/// Cached value from an LLM operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MemoValue {
    /// Generated summary text.
    Summary(String),

    /// Pilot navigation decision.
    PilotDecision(PilotDecisionValue),

    /// Query analysis result.
    QueryAnalysis(QueryAnalysisValue),

    /// Extracted content.
    Extraction(serde_json::Value),

    /// Raw text (for custom operations).
    Text(String),

    /// JSON value (for structured outputs).
    Json(serde_json::Value),
}

/// Serializable pilot decision value.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PilotDecisionValue {
    /// Selected candidate index.
    pub selected_idx: usize,

    /// Confidence score (0.0 to 1.0).
    pub confidence: f32,

    /// Reasoning text.
    pub reasoning: String,
}

/// Serializable query analysis value.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryAnalysisValue {
    /// Query complexity score.
    pub complexity: f32,

    /// Detected intent.
    pub intent: String,

    /// Suggested strategy.
    pub strategy: String,
}

impl MemoValue {
    /// Get the value as a string summary.
    pub fn as_summary(&self) -> Option<&str> {
        match self {
            MemoValue::Summary(s) => Some(s),
            _ => None,
        }
    }

    /// Get the value as text.
    pub fn as_text(&self) -> Option<&str> {
        match self {
            MemoValue::Text(s) => Some(s),
            MemoValue::Summary(s) => Some(s),
            _ => None,
        }
    }

    /// Check if this is a summary value.
    pub fn is_summary(&self) -> bool {
        matches!(self, MemoValue::Summary(_))
    }
}

/// A cached entry in the memo store.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoEntry {
    /// The cached value.
    pub value: MemoValue,

    /// When this entry was created.
    pub created_at: DateTime<Utc>,

    /// When this entry was last accessed.
    pub last_accessed: DateTime<Utc>,

    /// Number of cache hits.
    pub hits: u64,

    /// Token cost saved by this cache entry.
    pub tokens_saved: u64,
}

impl MemoEntry {
    /// Create a new entry.
    pub fn new(value: MemoValue) -> Self {
        let now = Utc::now();
        Self {
            value,
            created_at: now,
            last_accessed: now,
            hits: 0,
            tokens_saved: 0,
        }
    }

    /// Create a new entry with token count.
    pub fn with_tokens(value: MemoValue, tokens_saved: u64) -> Self {
        Self {
            tokens_saved,
            ..Self::new(value)
        }
    }

    /// Record a cache hit.
    pub fn record_hit(&mut self) {
        self.hits += 1;
        self.last_accessed = Utc::now();
    }

    /// Check if this entry has expired.
    pub fn is_expired(&self, ttl: chrono::Duration) -> bool {
        let now = Utc::now();
        now - self.created_at > ttl
    }

    /// Get the age of this entry.
    pub fn age(&self) -> chrono::Duration {
        Utc::now() - self.created_at
    }
}

/// Statistics for the memo store.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MemoStats {
    /// Total number of cache entries.
    pub entries: usize,

    /// Total cache hits.
    pub hits: u64,

    /// Total cache misses.
    pub misses: u64,

    /// Total tokens saved by cache hits.
    pub tokens_saved: u64,

    /// Estimated cost saved (in USD).
    pub cost_saved: f64,
}

impl MemoStats {
    /// Calculate the cache hit rate.
    pub fn hit_rate(&self) -> f64 {
        let total = self.hits + self.misses;
        if total == 0 {
            0.0
        } else {
            self.hits as f64 / total as f64
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memo_key_summary() {
        let fp = Fingerprint::from_str("test content");
        let key = MemoKey::summary(&fp);

        assert_eq!(key.op_type, MemoOpType::Summary);
        assert_eq!(key.input_fp, fp);
        assert!(key.model_id.is_none());
    }

    #[test]
    fn test_memo_key_with_model() {
        let fp = Fingerprint::from_str("test content");
        let key = MemoKey::summary(&fp).with_model("gpt-4").with_version(2);

        assert_eq!(key.model_id, Some("gpt-4".to_string()));
        assert_eq!(key.version, 2);
    }

    #[test]
    fn test_memo_key_fingerprint() {
        let fp = Fingerprint::from_str("test content");
        let key1 = MemoKey::summary(&fp);
        let key2 = MemoKey::summary(&fp);

        assert_eq!(key1.fingerprint(), key2.fingerprint());

        let key3 = MemoKey::summary_with_model(&fp, "gpt-4", 1);
        assert_ne!(key1.fingerprint(), key3.fingerprint());
    }

    #[test]
    fn test_memo_entry() {
        let entry = MemoEntry::new(MemoValue::Summary("Test summary".to_string()));

        assert_eq!(entry.hits, 0);
        assert!(entry.value.as_summary().is_some());
    }

    #[test]
    fn test_memo_entry_hit() {
        let mut entry = MemoEntry::new(MemoValue::Summary("Test summary".to_string()));
        entry.record_hit();
        entry.record_hit();

        assert_eq!(entry.hits, 2);
    }

    #[test]
    fn test_memo_stats_hit_rate() {
        let mut stats = MemoStats::default();
        stats.hits = 80;
        stats.misses = 20;

        assert!((stats.hit_rate() - 0.8).abs() < 0.001);
    }

    #[test]
    fn test_memo_key_serialization() {
        let fp = Fingerprint::from_str("test content");
        let key = MemoKey::summary_with_model(&fp, "gpt-4", 1);

        let json = serde_json::to_string(&key).unwrap();
        let decoded: MemoKey = serde_json::from_str(&json).unwrap();

        assert_eq!(key, decoded);
    }

    #[test]
    fn test_memo_value_serialization() {
        let value = MemoValue::Summary("Test summary".to_string());
        let json = serde_json::to_string(&value).unwrap();
        let decoded: MemoValue = serde_json::from_str(&json).unwrap();
        assert_eq!(value.as_summary(), decoded.as_summary());
    }
}
