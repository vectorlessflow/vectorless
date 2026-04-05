# Feedback Learning Design Document

> Pilot Feedback Learning System - Continuously improving decisions from user feedback

## Overview

Feedback Learning is Pilot's learning subsystem that continuously optimizes Pilot's decision-making capabilities by collecting user feedback on retrieval results. The system tracks decision accuracy across different scenarios and adjusts confidence levels and strategies for subsequent decisions accordingly.

### Design Goals

```
┌─────────────────────────────────────────────────────────────────┐
│                      Design Goals                                │
├─────────────────────────────────────────────────────────────────┤
│  1. Collect Feedback - Record user ratings on retrieval results  │
│  2. Learn Patterns - Identify scenarios where Pilot performs     │
│                      well or poorly                              │
│  3. Adjust Decisions - Modify confidence and strategies based    │
│                        on historical performance                 │
│  4. Continuous Improvement - Decision quality improves over time │
│                             as data accumulates                  │
└─────────────────────────────────────────────────────────────────┘
```

---

## 1. Overall Architecture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                      Feedback Learning System Architecture                    │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  ┌───────────────────────────────────────────────────────────────────────┐  │
│  │                        Data Flow                                       │  │
│  │                                                                       │  │
│  │   Retrieval Complete                                                  │  │
│  │      │                                                                │  │
│  │      ▼                                                                │  │
│  │   ┌─────────────┐     ┌─────────────┐     ┌─────────────┐           │  │
│  │   │   Feedback  │────▶│  Feedback   │────▶│    Pilot    │           │  │
│  │   │   Record    │     │   Store     │     │   Learner   │           │  │
│  │   └─────────────┘     └─────────────┘     └──────┬──────┘           │  │
│  │                                                   │                   │  │
│  │                                                   ▼                   │  │
│  │                                          ┌─────────────┐              │  │
│  │                                          │  Decision   │              │  │
│  │                                          │ Adjustment  │              │  │
│  │                                          └─────────────┘              │  │
│  │                                                   │                   │  │
│  │                                                   ▼                   │  │
│  │                                          Next Retrieval Decision       │  │
│  │                                                                       │  │
│  └───────────────────────────────────────────────────────────────────────┘  │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## 2. Core Components

### 2.1 FeedbackRecord - Feedback Record

```rust
/// Feedback record
pub struct FeedbackRecord {
    /// Unique feedback ID
    pub id: FeedbackId,
    /// Associated decision ID
    pub decision_id: DecisionId,
    /// Whether the decision was correct
    pub was_correct: bool,
    /// Pilot's confidence at that time
    pub pilot_confidence: f64,
    /// Intervention point type
    pub intervention_point: InterventionPoint,
    /// Query hash (for aggregating similar queries)
    pub query_hash: u64,
    /// Path hash (for aggregating similar paths)
    pub path_hash: u64,
    /// Timestamp
    pub timestamp_ms: u64,
    /// Optional user comment
    pub comment: Option<String>,
}
```

### 2.2 FeedbackStore - Feedback Storage

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                          FeedbackStore Architecture                          │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │  FeedbackStore                                                       │   │
│  │                                                                     │   │
│  │  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────────┐     │   │
│  │  │    records      │  │ intervention_   │  │   query_stats   │     │   │
│  │  │  Vec<Record>    │  │     stats       │  │  HashMap<u64,   │     │   │
│  │  │                 │  │                 │  │   ContextStats> │     │   │
│  │  └─────────────────┘  └─────────────────┘  └─────────────────┘     │   │
│  │                                                                     │   │
│  │  ┌─────────────────┐                                               │   │
│  │  │   path_stats    │                                               │   │
│  │  │  HashMap<u64,   │                                               │   │
│  │  │   ContextStats> │                                               │   │
│  │  └─────────────────┘                                               │   │
│  │                                                                     │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
│  Statistics Dimensions:                                                     │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │  1. Aggregate by InterventionPoint                                   │   │
│  │     - Accuracy for each: START / FORK / BACKTRACK / EVALUATE        │   │
│  │                                                                     │   │
│  │  2. Aggregate by Query                                              │   │
│  │     - Historical performance for similar queries                     │   │
│  │                                                                     │   │
│  │  3. Aggregate by Path                                               │   │
│  │     - Historical performance for similar paths                       │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 2.3 PilotLearner - Learner

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                          PilotLearner Workflow                               │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  Input:                                                                     │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │  - intervention_point: Current intervention point type               │   │
│  │  - query_hash: Hash value of the query                              │   │
│  │  - path_hash: Hash value of the path                                │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
│  Query Historical Statistics:                                               │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │  1. Get overall accuracy for intervention_point                      │   │
│  │  2. Get specific accuracy for query_hash (if available)              │   │
│  │  3. Get specific accuracy for path_hash (if available)               │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
│  Output DecisionAdjustment:                                                 │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │  pub struct DecisionAdjustment {                                    │   │
│  │      /// Confidence adjustment (added to Pilot confidence)           │   │
│  │      pub confidence_delta: f64,                                     │   │
│  │      /// Whether to skip intervention (trust algorithm)              │   │
│  │      pub skip_intervention: bool,                                   │   │
│  │      /// Algorithm weight vs LLM weight                             │   │
│  │      pub algorithm_weight: f64,                                     │   │
│  │  }                                                                  │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## 3. Learning Strategies

### 3.1 Accuracy Thresholds

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                          Accuracy Threshold Strategy                         │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  Configuration Parameters:                                                  │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │  min_samples: 10              // Minimum samples before adjusting    │   │
│  │  high_accuracy_threshold: 0.8 // High accuracy threshold             │   │
│  │  low_accuracy_threshold: 0.5  // Low accuracy threshold              │   │
│  │  max_confidence_delta: 0.2    // Maximum confidence adjustment       │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
│  Decision Logic:                                                            │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │                                                                     │   │
│  │  if accuracy >= high_accuracy_threshold (0.8):                      │   │
│  │      // High accuracy: trust LLM, boost confidence                  │   │
│  │      confidence_delta = +0.2                                        │   │
│  │      algorithm_weight = 0.3  // More reliance on LLM                │   │
│  │                                                                     │   │
│  │  elif accuracy <= low_accuracy_threshold (0.5):                     │   │
│  │      // Low accuracy: trust algorithm, reduce confidence            │   │
│  │      confidence_delta = -0.2                                        │   │
│  │      algorithm_weight = 0.7  // More reliance on algorithm          │   │
│  │                                                                     │   │
│  │      if accuracy < 0.3:                                             │   │
│  │          // Very low: skip LLM call, use algorithm only             │   │
│  │          skip_intervention = true                                   │   │
│  │                                                                     │   │
│  │  else:                                                              │   │
│  │      // Medium accuracy: keep defaults                              │   │
│  │      confidence_delta = 0.0                                         │   │
│  │      algorithm_weight = 0.5                                         │   │
│  │                                                                     │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 3.2 Multi-Layer Statistics Fusion

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                        Multi-Layer Statistics Fusion                         │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  Three-Layer Statistics:                                                    │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │                                                                     │   │
│  │  Layer 1: InterventionPoint Level (Coarse-grained)                  │   │
│  │  ┌─────────────────────────────────────────────────────────────┐   │   │
│  │  │  Example: FORK point overall accuracy = 0.75                 │   │   │
│  │  │  Impact: Base adjustment                                      │   │   │
│  │  └─────────────────────────────────────────────────────────────┘   │   │
│  │                                                                     │   │
│  │  Layer 2: Query Level (Medium-grained)                              │   │
│  │  ┌─────────────────────────────────────────────────────────────┐   │   │
│  │  │  Example: Similar query accuracy = 0.85                      │   │   │
│  │  │  Impact: If higher than overall, +0.05 confidence            │   │   │
│  │  └─────────────────────────────────────────────────────────────┘   │   │
│  │                                                                     │   │
│  │  Layer 3: Path Level (Fine-grained)                                 │   │
│  │  ┌─────────────────────────────────────────────────────────────┐   │   │
│  │  │  Example: Similar path accuracy = 0.92                       │   │   │
│  │  │  Impact: If very high, +0.05 confidence                       │   │   │
│  │  └─────────────────────────────────────────────────────────────┘   │   │
│  │                                                                     │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
│  Fusion Example:                                                            │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │                                                                     │   │
│  │  Scenario: FORK point, similar query, similar path                  │   │
│  │                                                                     │   │
│  │  1. FORK overall accuracy 0.75 → confidence_delta = +0.1            │   │
│  │  2. Query-specific accuracy 0.85 > 0.75 → confidence_delta += 0.05  │   │
│  │  3. Path-specific accuracy 0.92 > 0.9 → confidence_delta += 0.05    │   │
│  │                                                                     │   │
│  │  Final: confidence_delta = +0.2 (reached maximum)                   │   │
│  │                                                                     │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## 4. Integration with LlmPilot

### 4.1 Integration Points

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                        LlmPilot and Learner Integration                      │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  LlmPilot Structure:                                                        │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │  pub struct LlmPilot {                                              │   │
│  │      client: LlmClient,                                             │   │
│  │      executor: Option<Arc<LlmExecutor>>,                            │   │
│  │      config: PilotConfig,                                           │   │
│  │      budget: BudgetController,                                      │   │
│  │      context_builder: ContextBuilder,                               │   │
│  │      prompt_builder: PromptBuilder,                                 │   │
│  │      response_parser: ResponseParser,                               │   │
│  │      learner: Option<Arc<PilotLearner>>,  // ← Feedback learner     │   │
│  │  }                                                                  │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
│  Key Methods:                                                               │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │                                                                     │   │
│  │  // Add learner                                                     │   │
│  │  pub fn with_learner(self, learner: Arc<PilotLearner>) -> Self     │   │
│  │                                                                     │   │
│  │  // Create learner from feedback store                              │   │
│  │  pub fn with_feedback_store(self, store: Arc<FeedbackStore>) -> Self│   │
│  │                                                                     │   │
│  │  // Record feedback                                                 │   │
│  │  pub fn record_feedback(&self, record: FeedbackRecord)             │   │
│  │                                                                     │   │
│  │  // Get learner (read-only)                                         │   │
│  │  pub fn learner(&self) -> Option<&PilotLearner>                    │   │
│  │                                                                     │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 4.2 Decision Flow

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                          Decision Flow with Learning                         │
└─────────────────────────────────────────────────────────────────────────────┘

                    ┌─────────────────┐
                    │  call_llm()     │
                    └────────┬────────┘
                             │
                             ▼
              ┌──────────────────────────────┐
              │  1. Build Context (Builder)   │
              │     - query_section           │
              │     - path_section            │
              │     - candidates_section      │
              └──────────────┬───────────────┘
                             │
                             ▼
              ┌──────────────────────────────┐
              │  2. Get Learner Adjustment    │
              │     if learner.is_some() {    │
              │       query_hash = ctx.hash() │
              │       path_hash = ctx.hash()  │
              │       adjustment = learner    │
              │         .get_adjustment(      │
              │           point,              │
              │           query_hash,         │
              │           path_hash           │
              │         )                     │
              │     }                         │
              └──────────────┬───────────────┘
                             │
                             ▼
              ┌──────────────────────────────┐
              │  3. Check Skip Intervention   │
              │     if adjustment.skip {      │
              │       return default_decision │
              │     }                         │
              └──────────────┬───────────────┘
                             │
                             ▼
              ┌──────────────────────────────┐
              │  4. Call LLM for Decision     │
              │     decision = llm.complete() │
              └──────────────┬───────────────┘
                             │
                             ▼
              ┌──────────────────────────────┐
              │  5. Apply Learner Adjustment  │
              │     decision.confidence +=    │
              │       adjustment.confidence   │
              │       .delta                  │
              └──────────────┬───────────────┘
                             │
                             ▼
                    ┌─────────────────┐
                    │  Return Adjusted │
                    │     Decision     │
                    └─────────────────┘
```

---

## 5. Usage Examples

### 5.1 Basic Usage

```rust
use std::sync::Arc;
use vectorless::retrieval::pilot::{
    LlmPilot, PilotConfig,
    FeedbackStore, FeedbackRecord, PilotLearner,
};
use vectorless::llm::LlmClient;

// 1. Create feedback store
let store = Arc::new(FeedbackStore::in_memory());

// 2. Create Pilot with learner
let client = LlmClient::for_model("gpt-4o-mini");
let pilot = LlmPilot::new(client, PilotConfig::default())
    .with_feedback_store(store.clone());

// 3. Execute retrieval (Pilot automatically applies learning adjustments)
let decision = pilot.decide(&state).await;

// 4. Record user feedback
let record = FeedbackRecord::new(
    decision_id,
    was_correct,  // User rating
    decision.confidence as f64,
    InterventionPoint::Fork,
    query_hash,
    path_hash,
);
pilot.record_feedback(record);

// 5. Subsequent retrievals automatically leverage historical feedback
```

### 5.2 Persisting Feedback

```rust
use vectorless::retrieval::pilot::feedback::FeedbackStoreConfig;

// Create feedback store with persistence
let config = FeedbackStoreConfig::with_persistence("./data/feedback.json");
let store = Arc::new(FeedbackStore::new(config));

// Load historical feedback at startup
store.load()?;

// Persist periodically
store.persist()?;
```

### 5.3 Viewing Learning Effects

```rust
// Get overall accuracy
let accuracy = learner.overall_accuracy();
println!("Overall accuracy: {:.2}%", accuracy * 100.0);

// Get statistics by intervention point
let stats = store.intervention_stats();
println!("Fork accuracy: {:.2}%", stats.fork.accuracy() * 100.0);
println!("Start accuracy: {:.2}%", stats.start.accuracy() * 100.0);

// Check if sufficient data exists
if learner.has_sufficient_data() {
    println!("Learner has sufficient data for adjustments");
}
```

---

## 6. Configuration Options

```rust
/// Feedback store configuration
pub struct FeedbackStoreConfig {
    /// Maximum number of records (memory limit)
    pub max_records: usize,
    /// Whether to persist
    pub persist: bool,
    /// Persistence path
    pub storage_path: Option<String>,
}

/// Learner configuration
pub struct LearnerConfig {
    /// Minimum samples (no adjustment below this)
    pub min_samples: u64,
    /// High accuracy threshold
    pub high_accuracy_threshold: f64,
    /// Low accuracy threshold
    pub low_accuracy_threshold: f64,
    /// Maximum confidence adjustment
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
```

---

## 7. Implementation Details

### 7.1 Hash Calculation

```rust
impl PilotContext {
    /// Calculate query hash (for aggregating similar queries)
    pub fn query_hash(&self) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut hasher = DefaultHasher::new();
        self.query_section.hash(&mut hasher);
        hasher.finish()
    }

    /// Calculate path hash (for aggregating similar paths)
    pub fn path_hash(&self) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut hasher = DefaultHasher::new();
        self.path_section.hash(&mut hasher);
        hasher.finish()
    }
}
```

### 7.2 Statistics Calculation

```rust
impl ContextStats {
    /// Calculate accuracy
    pub fn accuracy(&self) -> f64 {
        if self.total == 0 {
            0.0
        } else {
            self.correct as f64 / self.total as f64
        }
    }

    /// Record new feedback (incremental update)
    fn record(&mut self, was_correct: bool, confidence: f64) {
        self.total += 1;
        if was_correct {
            self.correct += 1;
            // Incremental update of average confidence
            self.avg_confidence_correct = 
                (self.avg_confidence_correct * (self.correct - 1) as f64 + confidence)
                / self.correct as f64;
        } else {
            let incorrect = self.total - self.correct;
            self.avg_confidence_incorrect = 
                (self.avg_confidence_incorrect * (incorrect - 1) as f64 + confidence)
                / incorrect as f64;
        }
    }
}
```

---

## 8. Future Extensions

### 8.1 Potential Improvements

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                          Future Extension Directions                         │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  1. Semantic Similarity Aggregation                                         │
│     ┌─────────────────────────────────────────────────────────────────┐    │
│     │  Current: Aggregate using exact hash                             │    │
│     │  Future: Use embeddings to calculate semantic similarity,        │    │
│     │          aggregate semantically similar queries                  │    │
│     └─────────────────────────────────────────────────────────────────┘    │
│                                                                             │
│  2. Time Decay                                                              │
│     ┌─────────────────────────────────────────────────────────────────┐    │
│     │  Current: All historical feedback has equal weight               │    │
│     │  Future: Recent feedback has higher weight, old feedback         │    │
│     │          gradually decays                                        │    │
│     └─────────────────────────────────────────────────────────────────┘    │
│                                                                             │
│  3. Online Learning                                                         │
│     ┌─────────────────────────────────────────────────────────────────┐    │
│     │  Current: Offline analysis, online application                   │    │
│     │  Future: Real-time model parameter updates                       │    │
│     └─────────────────────────────────────────────────────────────────┘    │
│                                                                             │
│  4. Personalized Learning                                                   │
│     ┌─────────────────────────────────────────────────────────────────┐    │
│     │  Current: Global learning                                        │    │
│     │  Future: Learn separately per user/scenario                      │    │
│     └─────────────────────────────────────────────────────────────────┘    │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## 9. Code Structure

```
src/retrieval/pilot/
├── mod.rs              # Module entry point
├── feedback.rs         # FeedbackStore, PilotLearner implementation
├── llm_pilot.rs        # LlmPilot (integrates learner)
├── builder.rs          # ContextBuilder (adds hash methods)
└── ...
```
