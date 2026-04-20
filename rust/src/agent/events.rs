// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Agent-specific events for streaming and progress monitoring.
//!
//! Events are emitted through the agent's event sender during retrieval,
//! providing real-time visibility into navigation decisions, evidence
//! collection, and multi-document orchestration.

use serde::Serialize;

/// An event emitted during agent-based retrieval.
#[derive(Debug, Clone, Serialize)]
pub enum AgentEvent {
    /// Agent started a retrieval operation.
    Started {
        /// The query string.
        query: String,
        /// Whether this is a single-doc or multi-doc operation.
        multi_doc: bool,
    },

    /// Fast path triggered — keyword lookup returned a direct hit.
    FastPathHit {
        /// Matched keyword.
        keyword: String,
        /// Node title that matched.
        node_title: String,
        /// Confidence weight.
        weight: f32,
    },

    /// A navigation round completed.
    RoundCompleted {
        /// Round number (1-based).
        round: u32,
        /// Command that was executed.
        command: String,
        /// Whether the command succeeded.
        success: bool,
        /// Wall-clock time for this round in milliseconds.
        elapsed_ms: u64,
    },

    /// Evidence was collected from a node.
    EvidenceCollected {
        /// Node title.
        node_title: String,
        /// Navigation path to the node.
        source_path: String,
        /// Content length in characters.
        content_len: usize,
        /// Total evidence count so far.
        total_evidence: usize,
    },

    /// Sufficiency check result.
    SufficiencyCheck {
        /// Whether evidence is sufficient.
        sufficient: bool,
        /// Total evidence items.
        evidence_count: usize,
    },

    /// A navigation plan was generated (Phase 1.5).
    PlanGenerated {
        /// Document name.
        doc_name: String,
        /// Length of the generated plan text.
        plan_len: usize,
    },

    /// A re-plan was triggered after check returned INSUFFICIENT.
    ReplanGenerated {
        /// Document name.
        doc_name: String,
        /// What information was missing (triggers the re-plan).
        missing_info: String,
        /// Length of the new plan text.
        plan_len: usize,
    },

    /// A budget-related warning was injected (stuck detection or half-budget hint).
    BudgetWarning {
        /// Type of warning: "stuck" or "half_budget".
        warning_type: String,
        /// Current round number.
        round: u32,
    },

    /// Worker dispatched (orchestrator only).
    WorkerDispatched {
        /// Document index.
        doc_idx: usize,
        /// Document name.
        doc_name: String,
        /// Task assigned to the sub-agent.
        task: String,
    },

    /// Worker completed (orchestrator only).
    WorkerCompleted {
        /// Document index.
        doc_idx: usize,
        /// Number of evidence items collected.
        evidence_count: usize,
        /// Whether the sub-agent succeeded.
        success: bool,
    },

    /// Answer synthesis completed.
    SynthesisCompleted {
        /// Length of the synthesized answer.
        answer_len: usize,
    },

    /// Agent completed the entire retrieval.
    Completed {
        /// Final evidence count.
        evidence_count: usize,
        /// Total LLM calls made.
        llm_calls: u32,
        /// Total navigation rounds used.
        rounds_used: u32,
        /// Whether the fast-path was hit.
        fast_path_hit: bool,
        /// Whether the budget was exhausted.
        budget_exhausted: bool,
        /// Whether a navigation plan was generated.
        plan_generated: bool,
        /// Total characters of collected evidence.
        evidence_chars: usize,
    },

    /// An error occurred.
    Error {
        /// Error message.
        message: String,
    },
}

/// Sender for agent events.
pub(crate) type AgentEventSender = tokio::sync::mpsc::Sender<AgentEvent>;

/// Receiver for agent events.
pub type AgentEventReceiver = tokio::sync::mpsc::Receiver<AgentEvent>;

/// Create a bounded channel for agent events.
pub(crate) fn channel(bound: usize) -> (AgentEventSender, AgentEventReceiver) {
    tokio::sync::mpsc::channel(bound)
}

/// Default channel bound for agent events.
pub const DEFAULT_AGENT_EVENT_BOUND: usize = 128;

/// A handle for emitting agent events.
///
/// Wraps an `mpsc::Sender` and silently drops events if the receiver
/// is closed (no panic on send failure).
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

    /// Emit a started event.
    pub fn emit_started(&self, query: &str, multi_doc: bool) {
        self.emit(AgentEvent::Started {
            query: query.to_string(),
            multi_doc,
        });
    }

    /// Emit a fast-path hit event.
    pub fn emit_fast_path(&self, keyword: &str, node_title: &str, weight: f32) {
        self.emit(AgentEvent::FastPathHit {
            keyword: keyword.to_string(),
            node_title: node_title.to_string(),
            weight,
        });
    }

    /// Emit a round-completed event.
    pub fn emit_round(&self, round: u32, command: &str, success: bool, elapsed_ms: u64) {
        self.emit(AgentEvent::RoundCompleted {
            round,
            command: command.to_string(),
            success,
            elapsed_ms,
        });
    }

    /// Emit an evidence-collected event.
    pub fn emit_evidence(
        &self,
        node_title: &str,
        source_path: &str,
        content_len: usize,
        total: usize,
    ) {
        self.emit(AgentEvent::EvidenceCollected {
            node_title: node_title.to_string(),
            source_path: source_path.to_string(),
            content_len,
            total_evidence: total,
        });
    }

    /// Emit a sufficiency check event.
    pub fn emit_sufficiency(&self, sufficient: bool, evidence_count: usize) {
        self.emit(AgentEvent::SufficiencyCheck {
            sufficient,
            evidence_count,
        });
    }

    /// Emit a worker dispatched event.
    pub fn emit_worker_dispatched(&self, doc_idx: usize, doc_name: &str, task: &str) {
        self.emit(AgentEvent::WorkerDispatched {
            doc_idx,
            doc_name: doc_name.to_string(),
            task: task.to_string(),
        });
    }

    /// Emit a worker completed event.
    pub fn emit_worker_completed(&self, doc_idx: usize, evidence_count: usize, success: bool) {
        self.emit(AgentEvent::WorkerCompleted {
            doc_idx,
            evidence_count,
            success,
        });
    }

    /// Emit a synthesis completed event.
    pub fn emit_synthesis(&self, answer_len: usize) {
        self.emit(AgentEvent::SynthesisCompleted { answer_len });
    }

    /// Emit a completed event.
    pub fn emit_completed(
        &self,
        evidence_count: usize,
        llm_calls: u32,
        rounds_used: u32,
        fast_path_hit: bool,
        budget_exhausted: bool,
        plan_generated: bool,
        evidence_chars: usize,
    ) {
        self.emit(AgentEvent::Completed {
            evidence_count,
            llm_calls,
            rounds_used,
            fast_path_hit,
            budget_exhausted,
            plan_generated,
            evidence_chars,
        });
    }

    /// Emit a plan-generated event.
    pub fn emit_plan_generated(&self, doc_name: &str, plan_len: usize) {
        self.emit(AgentEvent::PlanGenerated {
            doc_name: doc_name.to_string(),
            plan_len,
        });
    }

    /// Emit a replan-generated event.
    pub fn emit_replan_generated(&self, doc_name: &str, missing_info: &str, plan_len: usize) {
        self.emit(AgentEvent::ReplanGenerated {
            doc_name: doc_name.to_string(),
            missing_info: missing_info.to_string(),
            plan_len,
        });
    }

    /// Emit a budget warning event.
    pub fn emit_budget_warning(&self, warning_type: &str, round: u32) {
        self.emit(AgentEvent::BudgetWarning {
            warning_type: warning_type.to_string(),
            round,
        });
    }

    /// Emit an error event.
    pub fn emit_error(&self, message: &str) {
        self.emit(AgentEvent::Error {
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
        emitter.emit_started("test", false);
        emitter.emit_round(1, "ls", true, 50);
        emitter.emit_completed(0, 0, 0, false, false, false, 0);
        emitter.emit_plan_generated("test", 42);
        emitter.emit_replan_generated("test", "missing data", 30);
        emitter.emit_budget_warning("stuck", 5);
        // No panic — events silently dropped
    }

    #[test]
    fn test_event_roundtrip() {
        let (tx, mut rx) = channel(DEFAULT_AGENT_EVENT_BOUND);
        let emitter = EventEmitter::new(tx);

        emitter.emit_started("what is X?", false);
        emitter.emit_evidence("Intro", "root/Intro", 100, 1);
        emitter.emit_sufficiency(true, 1);
        emitter.emit_completed(1, 3, 5, false, false, true, 100);

        let events: Vec<AgentEvent> = (0..4).map(|_| rx.blocking_recv().unwrap()).collect();

        assert!(matches!(&events[0], AgentEvent::Started { query, .. } if query == "what is X?"));
        assert!(
            matches!(&events[1], AgentEvent::EvidenceCollected { node_title, .. } if node_title == "Intro")
        );
        assert!(matches!(
            &events[2],
            AgentEvent::SufficiencyCheck {
                sufficient: true,
                ..
            }
        ));
        assert!(matches!(
            &events[3],
            AgentEvent::Completed {
                evidence_count: 1,
                plan_generated: true,
                ..
            }
        ));
    }

    #[test]
    fn test_serialization() {
        let event = AgentEvent::Started {
            query: "test".to_string(),
            multi_doc: false,
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("Started"));
        assert!(json.contains("test"));
    }
}
