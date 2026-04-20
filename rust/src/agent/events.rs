// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Agent events — rich, structured visibility into the entire retrieval pipeline.
//!
//! Events are organized by pipeline stage:
//! 1. **Query Understanding** — intent analysis, keyword extraction
//! 2. **Orchestrator** — document selection, dispatch, evaluation, replan
//! 3. **Worker** — navigation, evidence collection, budget management
//! 4. **Answer** — synthesis and fusion
//!
//! The stream terminates with `Completed` or `Error`.

use serde::Serialize;

/// An event emitted during agent-based retrieval.
///
/// Each variant carries the data a client needs to understand what happened,
/// not just that something happened. All events are `Clone + Serialize` so
/// they can be broadcast or persisted.
#[derive(Debug, Clone, Serialize)]
pub enum AgentEvent {
    // ── Query Understanding ──────────────────────────────────────────

    /// Query understanding started.
    QueryUnderstandingStarted {
        query: String,
    },

    /// Query understanding completed (intent, keywords, strategy decided).
    QueryUnderstandingCompleted {
        query: String,
        intent: String,
        keywords: Vec<String>,
        strategy_hint: String,
        complexity: String,
    },

    // ── Orchestrator ─────────────────────────────────────────────────

    /// Orchestrator started.
    OrchestratorStarted {
        query: String,
        doc_count: usize,
        skip_analysis: bool,
    },

    /// Orchestrator fast-path hit — keyword lookup answered directly.
    OrchestratorFastPath {
        keyword: String,
        doc_name: String,
        node_title: String,
        weight: f32,
    },

    /// Orchestrator is analyzing documents to select which to dispatch.
    OrchestratorAnalyzing {
        doc_count: usize,
        keywords: Vec<String>,
    },

    /// Orchestrator decided which documents to dispatch.
    OrchestratorPlanReady {
        dispatch_count: usize,
        /// (doc_idx, doc_name, task) for each dispatch.
        dispatches: Vec<(usize, String, String)>,
    },

    /// A Worker was dispatched to a document.
    WorkerDispatched {
        doc_idx: usize,
        doc_name: String,
        task: String,
        focus_keywords: Vec<String>,
    },

    /// A Worker finished its task.
    WorkerCompleted {
        doc_idx: usize,
        doc_name: String,
        evidence_count: usize,
        rounds_used: u32,
        llm_calls: u32,
        success: bool,
    },

    /// Cross-doc sufficiency evaluation result.
    OrchestratorEvaluated {
        sufficient: bool,
        evidence_count: usize,
        missing_info: Option<String>,
    },

    /// Orchestrator is replanning after insufficient evidence.
    OrchestratorReplanning {
        reason: String,
        evidence_count: usize,
    },

    /// Orchestrator completed.
    OrchestratorCompleted {
        evidence_count: usize,
        total_llm_calls: u32,
        dispatch_rounds: u32,
    },

    // ── Worker (per-document navigation) ─────────────────────────────

    /// Worker started on a document.
    WorkerStarted {
        doc_name: String,
        task: Option<String>,
        max_rounds: u32,
    },

    /// Worker fast-path hit.
    WorkerFastPath {
        doc_name: String,
        keyword: String,
        node_title: String,
        weight: f32,
    },

    /// Worker generated a navigation plan.
    WorkerPlanGenerated {
        doc_name: String,
        plan_len: usize,
    },

    /// A navigation round completed.
    WorkerRound {
        doc_name: String,
        round: u32,
        command: String,
        success: bool,
        elapsed_ms: u64,
    },

    /// Evidence was collected from a node.
    EvidenceCollected {
        doc_name: String,
        node_title: String,
        source_path: String,
        content_len: usize,
        total_evidence: usize,
    },

    /// Worker sufficiency check result.
    WorkerSufficiencyCheck {
        doc_name: String,
        sufficient: bool,
        evidence_count: usize,
        missing_info: Option<String>,
    },

    /// Worker re-planned after insufficient check.
    WorkerReplan {
        doc_name: String,
        missing_info: String,
        plan_len: usize,
    },

    /// Worker budget warning (stuck or half-budget).
    WorkerBudgetWarning {
        doc_name: String,
        warning_type: String,
        round: u32,
    },

    /// Worker completed.
    WorkerDone {
        doc_name: String,
        evidence_count: usize,
        rounds_used: u32,
        llm_calls: u32,
        budget_exhausted: bool,
        plan_generated: bool,
    },

    // ── Answer Pipeline ──────────────────────────────────────────────

    /// Answer synthesis started.
    AnswerStarted {
        evidence_count: usize,
        multi_doc: bool,
    },

    /// Answer synthesis completed.
    AnswerCompleted {
        answer_len: usize,
        confidence: String,
    },

    // ── Terminal ─────────────────────────────────────────────────────

    /// Entire retrieval pipeline completed.
    Completed {
        evidence_count: usize,
        llm_calls: u32,
        answer_len: usize,
    },

    /// An error occurred.
    Error {
        stage: String,
        message: String,
    },
}

// ---------------------------------------------------------------------------
// Channel + EventEmitter
// ---------------------------------------------------------------------------

/// Sender for agent events.
pub(crate) type AgentEventSender = tokio::sync::mpsc::Sender<AgentEvent>;

/// Receiver for agent events.
pub type AgentEventReceiver = tokio::sync::mpsc::Receiver<AgentEvent>;

/// Create a bounded channel for agent events.
pub(crate) fn channel(bound: usize) -> (AgentEventSender, AgentEventReceiver) {
    tokio::sync::mpsc::channel(bound)
}

/// Default channel bound for agent events.
pub const DEFAULT_AGENT_EVENT_BOUND: usize = 256;

/// A handle for emitting agent events.
///
/// Wraps an `mpsc::Sender` and silently drops events if the receiver
/// is closed (no panic on send failure). Cheaply clonable.
#[derive(Clone)]
pub struct EventEmitter {
    tx: Option<AgentEventSender>,
}

impl EventEmitter {
    /// Create a new emitter with the given sender.
    pub fn new(tx: AgentEventSender) -> Self {
        Self { tx: Some(tx) }
    }

    /// Create a noop emitter that discards all events.
    pub fn noop() -> Self {
        Self { tx: None }
    }

    /// Emit an event. Silently drops if the receiver is closed.
    pub fn emit(&self, event: AgentEvent) {
        if let Some(ref tx) = self.tx {
            let _ = tx.try_send(event);
        }
    }

    // ── Query Understanding ──

    pub fn emit_query_understanding_started(&self, query: &str) {
        self.emit(AgentEvent::QueryUnderstandingStarted {
            query: query.to_string(),
        });
    }

    pub fn emit_query_understanding_completed(
        &self,
        query: &str,
        intent: &str,
        keywords: &[String],
        strategy_hint: &str,
        complexity: &str,
    ) {
        self.emit(AgentEvent::QueryUnderstandingCompleted {
            query: query.to_string(),
            intent: intent.to_string(),
            keywords: keywords.to_vec(),
            strategy_hint: strategy_hint.to_string(),
            complexity: complexity.to_string(),
        });
    }

    // ── Orchestrator ──

    pub fn emit_orchestrator_started(&self, query: &str, doc_count: usize, skip_analysis: bool) {
        self.emit(AgentEvent::OrchestratorStarted {
            query: query.to_string(),
            doc_count,
            skip_analysis,
        });
    }

    pub fn emit_orchestrator_fast_path(
        &self,
        keyword: &str,
        doc_name: &str,
        node_title: &str,
        weight: f32,
    ) {
        self.emit(AgentEvent::OrchestratorFastPath {
            keyword: keyword.to_string(),
            doc_name: doc_name.to_string(),
            node_title: node_title.to_string(),
            weight,
        });
    }

    pub fn emit_orchestrator_analyzing(&self, doc_count: usize, keywords: &[String]) {
        self.emit(AgentEvent::OrchestratorAnalyzing {
            doc_count,
            keywords: keywords.to_vec(),
        });
    }

    pub fn emit_orchestrator_plan_ready(&self, dispatches: &[(usize, String, String)]) {
        self.emit(AgentEvent::OrchestratorPlanReady {
            dispatch_count: dispatches.len(),
            dispatches: dispatches.to_vec(),
        });
    }

    pub fn emit_worker_dispatched(
        &self,
        doc_idx: usize,
        doc_name: &str,
        task: &str,
        focus_keywords: &[String],
    ) {
        self.emit(AgentEvent::WorkerDispatched {
            doc_idx,
            doc_name: doc_name.to_string(),
            task: task.to_string(),
            focus_keywords: focus_keywords.to_vec(),
        });
    }

    pub fn emit_worker_completed(
        &self,
        doc_idx: usize,
        doc_name: &str,
        evidence_count: usize,
        rounds_used: u32,
        llm_calls: u32,
        success: bool,
    ) {
        self.emit(AgentEvent::WorkerCompleted {
            doc_idx,
            doc_name: doc_name.to_string(),
            evidence_count,
            rounds_used,
            llm_calls,
            success,
        });
    }

    pub fn emit_orchestrator_evaluated(
        &self,
        sufficient: bool,
        evidence_count: usize,
        missing_info: Option<&str>,
    ) {
        self.emit(AgentEvent::OrchestratorEvaluated {
            sufficient,
            evidence_count,
            missing_info: missing_info.map(|s| s.to_string()),
        });
    }

    pub fn emit_orchestrator_replanning(&self, reason: &str, evidence_count: usize) {
        self.emit(AgentEvent::OrchestratorReplanning {
            reason: reason.to_string(),
            evidence_count,
        });
    }

    pub fn emit_orchestrator_completed(
        &self,
        evidence_count: usize,
        total_llm_calls: u32,
        dispatch_rounds: u32,
    ) {
        self.emit(AgentEvent::OrchestratorCompleted {
            evidence_count,
            total_llm_calls,
            dispatch_rounds,
        });
    }

    // ── Worker ──

    pub fn emit_worker_started(&self, doc_name: &str, task: Option<&str>, max_rounds: u32) {
        self.emit(AgentEvent::WorkerStarted {
            doc_name: doc_name.to_string(),
            task: task.map(|s| s.to_string()),
            max_rounds,
        });
    }

    pub fn emit_worker_fast_path(
        &self,
        doc_name: &str,
        keyword: &str,
        node_title: &str,
        weight: f32,
    ) {
        self.emit(AgentEvent::WorkerFastPath {
            doc_name: doc_name.to_string(),
            keyword: keyword.to_string(),
            node_title: node_title.to_string(),
            weight,
        });
    }

    pub fn emit_worker_plan_generated(&self, doc_name: &str, plan_len: usize) {
        self.emit(AgentEvent::WorkerPlanGenerated {
            doc_name: doc_name.to_string(),
            plan_len,
        });
    }

    pub fn emit_worker_round(
        &self,
        doc_name: &str,
        round: u32,
        command: &str,
        success: bool,
        elapsed_ms: u64,
    ) {
        self.emit(AgentEvent::WorkerRound {
            doc_name: doc_name.to_string(),
            round,
            command: command.to_string(),
            success,
            elapsed_ms,
        });
    }

    pub fn emit_evidence(
        &self,
        doc_name: &str,
        node_title: &str,
        source_path: &str,
        content_len: usize,
        total: usize,
    ) {
        self.emit(AgentEvent::EvidenceCollected {
            doc_name: doc_name.to_string(),
            node_title: node_title.to_string(),
            source_path: source_path.to_string(),
            content_len,
            total_evidence: total,
        });
    }

    pub fn emit_worker_sufficiency_check(
        &self,
        doc_name: &str,
        sufficient: bool,
        evidence_count: usize,
        missing_info: Option<&str>,
    ) {
        self.emit(AgentEvent::WorkerSufficiencyCheck {
            doc_name: doc_name.to_string(),
            sufficient,
            evidence_count,
            missing_info: missing_info.map(|s| s.to_string()),
        });
    }

    pub fn emit_worker_replan(&self, doc_name: &str, missing_info: &str, plan_len: usize) {
        self.emit(AgentEvent::WorkerReplan {
            doc_name: doc_name.to_string(),
            missing_info: missing_info.to_string(),
            plan_len,
        });
    }

    pub fn emit_worker_budget_warning(&self, doc_name: &str, warning_type: &str, round: u32) {
        self.emit(AgentEvent::WorkerBudgetWarning {
            doc_name: doc_name.to_string(),
            warning_type: warning_type.to_string(),
            round,
        });
    }

    pub fn emit_worker_done(
        &self,
        doc_name: &str,
        evidence_count: usize,
        rounds_used: u32,
        llm_calls: u32,
        budget_exhausted: bool,
        plan_generated: bool,
    ) {
        self.emit(AgentEvent::WorkerDone {
            doc_name: doc_name.to_string(),
            evidence_count,
            rounds_used,
            llm_calls,
            budget_exhausted,
            plan_generated,
        });
    }

    // ── Answer ──

    pub fn emit_answer_started(&self, evidence_count: usize, multi_doc: bool) {
        self.emit(AgentEvent::AnswerStarted {
            evidence_count,
            multi_doc,
        });
    }

    pub fn emit_answer_completed(&self, answer_len: usize, confidence: &str) {
        self.emit(AgentEvent::AnswerCompleted {
            answer_len,
            confidence: confidence.to_string(),
        });
    }

    // ── Terminal ──

    pub fn emit_completed(&self, evidence_count: usize, llm_calls: u32, answer_len: usize) {
        self.emit(AgentEvent::Completed {
            evidence_count,
            llm_calls,
            answer_len,
        });
    }

    pub fn emit_error(&self, stage: &str, message: &str) {
        self.emit(AgentEvent::Error {
            stage: stage.to_string(),
            message: message.to_string(),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_noop_emitter() {
        let emitter = EventEmitter::noop();
        emitter.emit_orchestrator_started("test", 1, false);
        emitter.emit_worker_started("doc.md", None, 8);
        emitter.emit_worker_round("doc.md", 1, "ls", true, 50);
        emitter.emit_worker_done("doc.md", 0, 1, 1, false, false);
        emitter.emit_completed(0, 1, 0);
        // No panic — events silently dropped
    }

    #[test]
    fn test_event_roundtrip() {
        let (tx, mut rx) = channel(DEFAULT_AGENT_EVENT_BOUND);
        let emitter = EventEmitter::new(tx);

        emitter.emit_orchestrator_started("what is X?", 1, true);
        emitter.emit_worker_started("doc.md", None, 8);
        emitter.emit_evidence("doc.md", "Intro", "root/Intro", 100, 1);
        emitter.emit_worker_sufficiency_check("doc.md", true, 1, None);
        emitter.emit_worker_done("doc.md", 1, 3, 5, false, true);
        emitter.emit_completed(1, 6, 42);

        let events: Vec<AgentEvent> = (0..6).map(|_| rx.blocking_recv().unwrap()).collect();

        assert!(matches!(&events[0], AgentEvent::OrchestratorStarted { query, .. } if query == "what is X?"));
        assert!(matches!(&events[1], AgentEvent::WorkerStarted { doc_name, .. } if doc_name == "doc.md"));
        assert!(matches!(&events[2], AgentEvent::EvidenceCollected { node_title, .. } if node_title == "Intro"));
        assert!(matches!(&events[3], AgentEvent::WorkerSufficiencyCheck { sufficient: true, .. }));
        assert!(matches!(&events[4], AgentEvent::WorkerDone { evidence_count: 1, plan_generated: true, .. }));
        assert!(matches!(&events[5], AgentEvent::Completed { evidence_count: 1, answer_len: 42, .. }));
    }

    #[test]
    fn test_serialization() {
        let event = AgentEvent::OrchestratorStarted {
            query: "test".to_string(),
            doc_count: 3,
            skip_analysis: false,
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("OrchestratorStarted"));
        assert!(json.contains("test"));
    }
}
