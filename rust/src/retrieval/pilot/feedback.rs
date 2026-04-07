// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Pilot feedback learning system.
//!
//! This module provides feedback collection and learning capabilities
//! for the Pilot to improve its decision-making over time.
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                    Feedback Learning System                      │
//! ├─────────────────────────────────────────────────────────────────┤
//! │                                                                  │
//! │   ┌─────────────┐   ┌─────────────┐   ┌─────────────┐          │
//! │   │  Feedback   │   │  Feedback   │   │    Pilot    │          │
//! │   │  Record     │──▶│   Store     │──▶│   Learner   │          │
//! │   └─────────────┘   └─────────────┘   └─────────────┘          │
//! │                            │                │                   │
//! │                            ▼                ▼                   │
//! │                     [Persistence]    [Decision Adjustment]      │
//! │                                                                  │
//! └─────────────────────────────────────────────────────────────────┘
//! ```
//!
//! # Example
//!
//! ```rust,ignore
//! use vectorless::retrieval::pilot::feedback::{FeedbackStore, FeedbackRecord};
//!
//! let store = FeedbackStore::new("./feedback_store");
//!
//! // Record feedback
//! let record = FeedbackRecord::new(decision_id, was_correct, confidence);
//! store.record(record).await?;
//!
//! // Learn from feedback
//! let learner = PilotLearner::new(store);
//! let adjustment = learner.get_adjustment(&context);
//! ```

use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};

use super::decision::InterventionPoint;

/// Unique identifier for a feedback record.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FeedbackId(pub u64);

/// Unique identifier for a decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DecisionId(pub u64);

/// Feedback record for a Pilot decision.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeedbackRecord {
    /// Unique feedback ID.
    pub id: FeedbackId,
    /// Associated decision ID.
    pub decision_id: DecisionId,
    /// Whether the decision was correct.
    pub was_correct: bool,
    /// Pilot's confidence at decision time.
    pub pilot_confidence: f64,
    /// Intervention point type.
    pub intervention_point: InterventionPoint,
    /// Query hash for grouping similar queries.
    pub query_hash: u64,
    /// Node path hash for context.
    pub path_hash: u64,
    /// Timestamp of feedback.
    pub timestamp_ms: u64,
    /// Optional user comment.
    pub comment: Option<String>,
}

impl FeedbackRecord {
    /// Create a new feedback record.
    pub fn new(
        decision_id: DecisionId,
        was_correct: bool,
        pilot_confidence: f64,
        intervention_point: InterventionPoint,
        query_hash: u64,
        path_hash: u64,
    ) -> Self {
        static COUNTER: AtomicU64 = AtomicU64::new(1);
        let id = FeedbackId(COUNTER.fetch_add(1, Ordering::Relaxed));
        let timestamp_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);

        Self {
            id,
            decision_id,
            was_correct,
            pilot_confidence,
            intervention_point,
            query_hash,
            path_hash,
            timestamp_ms,
            comment: None,
        }
    }

    /// Add a comment to the feedback.
    pub fn with_comment(mut self, comment: impl Into<String>) -> Self {
        self.comment = Some(comment.into());
        self
    }
}

/// Statistics for a specific context (query/path combination).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ContextStats {
    /// Total decisions in this context.
    pub total: u64,
    /// Correct decisions in this context.
    pub correct: u64,
    /// Average confidence when correct.
    pub avg_confidence_correct: f64,
    /// Average confidence when incorrect.
    pub avg_confidence_incorrect: f64,
}

impl ContextStats {
    /// Get accuracy for this context.
    pub fn accuracy(&self) -> f64 {
        if self.total == 0 {
            0.0
        } else {
            self.correct as f64 / self.total as f64
        }
    }

    /// Record a new feedback.
    fn record(&mut self, was_correct: bool, confidence: f64) {
        self.total += 1;
        if was_correct {
            self.correct += 1;
            // Running average
            self.avg_confidence_correct = (self.avg_confidence_correct
                * (self.correct - 1) as f64
                + confidence)
                / self.correct as f64;
        } else {
            let incorrect = self.total - self.correct;
            self.avg_confidence_incorrect = (self.avg_confidence_incorrect
                * (incorrect - 1) as f64
                + confidence)
                / incorrect as f64;
        }
    }
}

/// Statistics for an intervention point type.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct InterventionStats {
    /// Start intervention stats.
    pub start: ContextStats,
    /// Fork intervention stats.
    pub fork: ContextStats,
    /// Backtrack intervention stats.
    pub backtrack: ContextStats,
    /// Evaluate intervention stats.
    pub evaluate: ContextStats,
}

impl InterventionStats {
    /// Get stats for a specific intervention point.
    pub fn get(&self, point: InterventionPoint) -> &ContextStats {
        match point {
            InterventionPoint::Start => &self.start,
            InterventionPoint::Fork => &self.fork,
            InterventionPoint::Backtrack => &self.backtrack,
            InterventionPoint::Evaluate => &self.evaluate,
        }
    }

    /// Get mutable stats for a specific intervention point.
    fn get_mut(&mut self, point: InterventionPoint) -> &mut ContextStats {
        match point {
            InterventionPoint::Start => &mut self.start,
            InterventionPoint::Fork => &mut self.fork,
            InterventionPoint::Backtrack => &mut self.backtrack,
            InterventionPoint::Evaluate => &mut self.evaluate,
        }
    }
}

/// In-memory feedback store.
///
/// Stores feedback records and provides statistics for learning.
/// Thread-safe for concurrent access.
#[derive(Debug)]
pub struct FeedbackStore {
    /// All feedback records.
    records: std::sync::RwLock<Vec<FeedbackRecord>>,
    /// Statistics by intervention point.
    intervention_stats: std::sync::RwLock<InterventionStats>,
    /// Statistics by query hash.
    query_stats: std::sync::RwLock<HashMap<u64, ContextStats>>,
    /// Statistics by path hash.
    path_stats: std::sync::RwLock<HashMap<u64, ContextStats>>,
    /// Configuration.
    config: FeedbackStoreConfig,
}

/// Configuration for feedback store.
#[derive(Debug, Clone)]
pub struct FeedbackStoreConfig {
    /// Maximum records to keep in memory.
    pub max_records: usize,
    /// Enable persistence to disk.
    pub persist: bool,
    /// Path for persistence.
    pub storage_path: Option<String>,
}

impl Default for FeedbackStoreConfig {
    fn default() -> Self {
        Self {
            max_records: 10_000,
            persist: false,
            storage_path: None,
        }
    }
}

impl FeedbackStoreConfig {
    /// Create config with persistence enabled.
    pub fn with_persistence(path: impl Into<String>) -> Self {
        Self {
            max_records: 10_000,
            persist: true,
            storage_path: Some(path.into()),
        }
    }
}

impl FeedbackStore {
    /// Create a new feedback store.
    pub fn new(config: FeedbackStoreConfig) -> Self {
        Self {
            records: std::sync::RwLock::new(Vec::new()),
            intervention_stats: std::sync::RwLock::new(InterventionStats::default()),
            query_stats: std::sync::RwLock::new(HashMap::new()),
            path_stats: std::sync::RwLock::new(HashMap::new()),
            config,
        }
    }

    /// Create an in-memory store without persistence.
    pub fn in_memory() -> Self {
        Self::new(FeedbackStoreConfig::default())
    }

    /// Record a feedback.
    pub fn record(&self, feedback: FeedbackRecord) {
        // Update intervention stats
        {
            let mut stats = self.intervention_stats.write().unwrap();
            stats
                .get_mut(feedback.intervention_point)
                .record(feedback.was_correct, feedback.pilot_confidence);
        }

        // Update query stats
        {
            let mut stats = self.query_stats.write().unwrap();
            stats
                .entry(feedback.query_hash)
                .or_default()
                .record(feedback.was_correct, feedback.pilot_confidence);
        }

        // Update path stats
        {
            let mut stats = self.path_stats.write().unwrap();
            stats
                .entry(feedback.path_hash)
                .or_default()
                .record(feedback.was_correct, feedback.pilot_confidence);
        }

        // Store record
        {
            let mut records = self.records.write().unwrap();
            records.push(feedback);

            // Enforce max records limit
            if records.len() > self.config.max_records {
                let remove_count = records.len() - self.config.max_records;
                records.drain(0..remove_count);
            }
        }

        debug!(
            total_records = self.records.read().unwrap().len(),
            "Recorded feedback"
        );
    }

    /// Get overall intervention statistics.
    pub fn intervention_stats(&self) -> InterventionStats {
        self.intervention_stats.read().unwrap().clone()
    }

    /// Get statistics for a specific query hash.
    pub fn query_stats(&self, query_hash: u64) -> Option<ContextStats> {
        self.query_stats.read().unwrap().get(&query_hash).cloned()
    }

    /// Get statistics for a specific path hash.
    pub fn path_stats(&self, path_hash: u64) -> Option<ContextStats> {
        self.path_stats.read().unwrap().get(&path_hash).cloned()
    }

    /// Get total number of feedback records.
    pub fn total_records(&self) -> usize {
        self.records.read().unwrap().len()
    }

    /// Get overall accuracy across all feedback.
    pub fn overall_accuracy(&self) -> f64 {
        let stats = self.intervention_stats.read().unwrap();
        let total = stats.start.total
            + stats.fork.total
            + stats.backtrack.total
            + stats.evaluate.total;
        let correct = stats.start.correct
            + stats.fork.correct
            + stats.backtrack.correct
            + stats.evaluate.correct;

        if total == 0 {
            0.0
        } else {
            correct as f64 / total as f64
        }
    }

    /// Clear all feedback records.
    pub fn clear(&self) {
        self.records.write().unwrap().clear();
        *self.intervention_stats.write().unwrap() = InterventionStats::default();
        self.query_stats.write().unwrap().clear();
        self.path_stats.write().unwrap().clear();
    }

    /// Persist feedback to disk (if configured).
    pub fn persist(&self) -> std::io::Result<()> {
        if !self.config.persist {
            return Ok(());
        }

        let path = self.config.storage_path.as_ref().ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::NotFound, "No storage path configured")
        })?;

        let records = self.records.read().unwrap();
        let json = serde_json::to_string_pretty(&*records)?;
        std::fs::write(path, json)?;

        info!(path = %path, records = records.len(), "Persisted feedback store");
        Ok(())
    }

    /// Load feedback from disk (if configured).
    pub fn load(&self) -> std::io::Result<()> {
        if !self.config.persist {
            return Ok(());
        }

        let path = self.config.storage_path.as_ref().ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::NotFound, "No storage path configured")
        })?;

        if !Path::new(path).exists() {
            return Ok(());
        }

        let json = std::fs::read_to_string(path)?;
        let records: Vec<FeedbackRecord> = serde_json::from_str(&json)?;

        // Rebuild stats from records
        for record in &records {
            // Update intervention stats
            self.intervention_stats
                .write()
                .unwrap()
                .get_mut(record.intervention_point)
                .record(record.was_correct, record.pilot_confidence);

            // Update query stats
            self.query_stats
                .write()
                .unwrap()
                .entry(record.query_hash)
                .or_default()
                .record(record.was_correct, record.pilot_confidence);

            // Update path stats
            self.path_stats
                .write()
                .unwrap()
                .entry(record.path_hash)
                .or_default()
                .record(record.was_correct, record.pilot_confidence);
        }

        *self.records.write().unwrap() = records;

        info!(path = %path, "Loaded feedback store");
        Ok(())
    }
}

/// Decision adjustment based on learned feedback.
#[derive(Debug, Clone, Copy)]
pub struct DecisionAdjustment {
    /// Confidence adjustment (add to pilot confidence).
    pub confidence_delta: f64,
    /// Whether to skip intervention (algorithm is confident).
    pub skip_intervention: bool,
    /// Weight to apply to algorithm score vs LLM score.
    pub algorithm_weight: f64,
}

impl Default for DecisionAdjustment {
    fn default() -> Self {
        Self {
            confidence_delta: 0.0,
            skip_intervention: false,
            algorithm_weight: 0.5,
        }
    }
}

/// Pilot learner that adjusts decisions based on feedback.
///
/// Uses collected feedback to:
/// 1. Adjust confidence thresholds for different intervention points
/// 2. Decide when to skip intervention (trust algorithm)
/// 3. Adjust the weight between algorithm and LLM scores
#[derive(Debug)]
pub struct PilotLearner {
    /// Feedback store reference.
    store: Arc<FeedbackStore>,
    /// Learning configuration.
    config: LearnerConfig,
}

/// Configuration for the pilot learner.
#[derive(Debug, Clone)]
pub struct LearnerConfig {
    /// Minimum samples required before adjusting.
    pub min_samples: u64,
    /// Threshold for high accuracy (trust LLM more).
    pub high_accuracy_threshold: f64,
    /// Threshold for low accuracy (trust algorithm more).
    pub low_accuracy_threshold: f64,
    /// Maximum confidence adjustment.
    pub max_confidence_delta: f64,
}

impl Default for LearnerConfig {
    fn default() -> Self {
        Self {
            min_samples: 10,
            high_accuracy_threshold: 0.8,
            low_accuracy_threshold: 0.5,
            max_confidence_delta: 0.2,
        }
    }
}

impl PilotLearner {
    /// Create a new learner with the given feedback store.
    pub fn new(store: Arc<FeedbackStore>) -> Self {
        Self {
            store,
            config: LearnerConfig::default(),
        }
    }

    /// Create a learner with custom configuration.
    pub fn with_config(store: Arc<FeedbackStore>, config: LearnerConfig) -> Self {
        Self { store, config }
    }

    /// Get decision adjustment for a given context.
    pub fn get_adjustment(
        &self,
        intervention_point: InterventionPoint,
        query_hash: u64,
        path_hash: u64,
    ) -> DecisionAdjustment {
        let mut adjustment = DecisionAdjustment::default();

        // Get intervention-level stats
        let intervention_stats = self.store.intervention_stats();
        let point_stats = intervention_stats.get(intervention_point);

        // Not enough samples, use defaults
        if point_stats.total < self.config.min_samples {
            return adjustment;
        }

        let accuracy = point_stats.accuracy();

        // Adjust based on accuracy
        if accuracy >= self.config.high_accuracy_threshold {
            // High accuracy: trust LLM more
            adjustment.confidence_delta = self.config.max_confidence_delta;
            adjustment.algorithm_weight = 0.3; // Favor LLM
        } else if accuracy <= self.config.low_accuracy_threshold {
            // Low accuracy: trust algorithm more
            adjustment.confidence_delta = -self.config.max_confidence_delta;
            adjustment.algorithm_weight = 0.7; // Favor algorithm
            adjustment.skip_intervention = accuracy < 0.3; // Very low accuracy, skip LLM
        }

        // Further refine based on query-specific stats
        if let Some(query_stats) = self.store.query_stats(query_hash) {
            if query_stats.total >= self.config.min_samples {
                let query_accuracy = query_stats.accuracy();
                // Adjust confidence based on query-specific performance
                if query_accuracy > accuracy {
                    adjustment.confidence_delta += 0.05;
                } else if query_accuracy < accuracy {
                    adjustment.confidence_delta -= 0.05;
                }
            }
        }

        // Further refine based on path-specific stats
        if let Some(path_stats) = self.store.path_stats(path_hash) {
            if path_stats.total >= self.config.min_samples {
                let path_accuracy = path_stats.accuracy();
                // If this path has very high accuracy, increase confidence
                if path_accuracy > 0.9 {
                    adjustment.confidence_delta += 0.05;
                }
            }
        }

        // Clamp confidence delta
        adjustment.confidence_delta = adjustment
            .confidence_delta
            .clamp(-self.config.max_confidence_delta, self.config.max_confidence_delta);

        adjustment
    }

    /// Get the feedback store.
    pub fn store(&self) -> &FeedbackStore {
        &self.store
    }

    /// Get overall accuracy.
    pub fn overall_accuracy(&self) -> f64 {
        self.store.overall_accuracy()
    }

    /// Check if enough feedback has been collected.
    pub fn has_sufficient_data(&self) -> bool {
        let stats = self.store.intervention_stats();
        let total = stats.start.total
            + stats.fork.total
            + stats.backtrack.total
            + stats.evaluate.total;
        total >= self.config.min_samples
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_hash(s: &str) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut hasher = DefaultHasher::new();
        s.hash(&mut hasher);
        hasher.finish()
    }

    #[test]
    fn test_feedback_record_creation() {
        let record = FeedbackRecord::new(
            DecisionId(1),
            true,
            0.85,
            InterventionPoint::Fork,
            make_hash("test query"),
            make_hash("/root/child"),
        );

        assert!(record.was_correct);
        assert!((record.pilot_confidence - 0.85).abs() < 0.01);
        assert!(record.comment.is_none());
    }

    #[test]
    fn test_feedback_record_with_comment() {
        let record = FeedbackRecord::new(
            DecisionId(1),
            false,
            0.5,
            InterventionPoint::Start,
            make_hash("test"),
            make_hash("/"),
        )
        .with_comment("Wrong direction");

        assert!(!record.was_correct);
        assert_eq!(record.comment, Some("Wrong direction".to_string()));
    }

    #[test]
    fn test_feedback_store_recording() {
        let store = FeedbackStore::in_memory();

        // Record some feedback
        store.record(FeedbackRecord::new(
            DecisionId(1),
            true,
            0.9,
            InterventionPoint::Fork,
            make_hash("query1"),
            make_hash("/path1"),
        ));

        store.record(FeedbackRecord::new(
            DecisionId(2),
            false,
            0.6,
            InterventionPoint::Fork,
            make_hash("query1"),
            make_hash("/path1"),
        ));

        store.record(FeedbackRecord::new(
            DecisionId(3),
            true,
            0.8,
            InterventionPoint::Start,
            make_hash("query2"),
            make_hash("/"),
        ));

        assert_eq!(store.total_records(), 3);

        let stats = store.intervention_stats();
        assert_eq!(stats.fork.total, 2);
        assert_eq!(stats.fork.correct, 1);
        assert!((stats.fork.accuracy() - 0.5).abs() < 0.01);

        assert_eq!(stats.start.total, 1);
        assert_eq!(stats.start.correct, 1);
    }

    #[test]
    fn test_pilot_learner_adjustment() {
        let store = Arc::new(FeedbackStore::in_memory());
        let learner = PilotLearner::new(store.clone());

        // Not enough data, should return default
        let adj = learner.get_adjustment(InterventionPoint::Fork, 0, 0);
        assert!((adj.confidence_delta - 0.0).abs() < 0.01);
        assert!(!adj.skip_intervention);

        // Add enough feedback with high accuracy
        for i in 0..15 {
            store.record(FeedbackRecord::new(
                DecisionId(i),
                true, // All correct
                0.9,
                InterventionPoint::Fork,
                make_hash("query"),
                make_hash("/path"),
            ));
        }

        // Now should adjust
        let adj = learner.get_adjustment(InterventionPoint::Fork, make_hash("query"), 0);
        assert!(adj.confidence_delta > 0.0); // Should boost confidence
        assert!((adj.algorithm_weight - 0.3).abs() < 0.01); // Should favor LLM
    }

    #[test]
    fn test_pilot_learner_low_accuracy() {
        let store = Arc::new(FeedbackStore::in_memory());
        let learner = PilotLearner::new(store.clone());

        // Add enough feedback with low accuracy
        for i in 0..15 {
            store.record(FeedbackRecord::new(
                DecisionId(i),
                i % 3 == 0, // Only ~33% correct
                0.5,
                InterventionPoint::Fork,
                0,
                0,
            ));
        }

        let adj = learner.get_adjustment(InterventionPoint::Fork, 0, 0);
        assert!(adj.confidence_delta < 0.0); // Should reduce confidence
        assert!(adj.algorithm_weight > 0.5); // Should favor algorithm
    }

    #[test]
    fn test_context_stats() {
        let mut stats = ContextStats::default();

        stats.record(true, 0.9);
        stats.record(true, 0.8);
        stats.record(false, 0.6);

        assert_eq!(stats.total, 3);
        assert_eq!(stats.correct, 2);
        assert!((stats.accuracy() - 0.666).abs() < 0.01);
        assert!((stats.avg_confidence_correct - 0.85).abs() < 0.01);
        assert!((stats.avg_confidence_incorrect - 0.6).abs() < 0.01);
    }
}
