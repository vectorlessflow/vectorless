// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Post-processing of agent output into client-facing results.
//!
//! Converts raw agent [`Output`] into one or more [`QueryResultItem`]s.
//! When evidence comes from multiple documents (distinct `doc_name` values),
//! results are split by document so the caller can see per-doc attribution.

use std::collections::BTreeMap;

use crate::agent::config::{Evidence, Metrics, Output};
use crate::client::{Confidence, EvidenceItem, QueryMetrics, QueryResultItem};
use crate::rerank::types::ConfidenceLevel;

/// Convert agent output to query result items, split by document.
///
/// Groups evidence by `doc_name` and creates one `QueryResultItem` per document.
/// For single-document queries (all evidence has the same or no `doc_name`),
/// returns a single item with the given `doc_id`.
///
/// The synthesized answer is shared across all items (it was produced from
/// cross-document evidence). Each item gets its own subset of evidence.
pub fn to_results(output: &Output, doc_id: &str) -> Vec<QueryResultItem> {
    if output.evidence.is_empty() {
        return vec![empty_item(doc_id, &output.answer, output.score)];
    }

    // Group evidence by doc_name
    let groups = group_by_doc(&output.evidence);

    if groups.len() <= 1 {
        // Single doc — return one item
        return vec![build_item(
            doc_id,
            &output.answer,
            output.score,
            &output.evidence,
            &output.metrics,
        )];
    }

    // Multi-doc — one item per document
    groups
        .into_iter()
        .map(|(name, refs)| {
            let did = name.as_deref().unwrap_or(doc_id);
            let evidence: Vec<Evidence> = refs.iter().map(|e| (*e).clone()).collect();
            build_item(did, &output.answer, output.score, &evidence, &output.metrics)
        })
        .collect()
}

/// Group evidence by `doc_name`, preserving order.
fn group_by_doc(evidence: &[Evidence]) -> BTreeMap<Option<String>, Vec<&Evidence>> {
    let mut groups: BTreeMap<Option<String>, Vec<&Evidence>> = BTreeMap::new();
    for ev in evidence {
        groups.entry(ev.doc_name.clone()).or_default().push(ev);
    }
    groups
}

/// Build a single enriched result item.
fn build_item(
    doc_id: &str,
    answer: &str,
    score: f32,
    evidence: &[Evidence],
    metrics: &Metrics,
) -> QueryResultItem {
    let node_ids: Vec<String> = evidence.iter().map(|e| e.source_path.clone()).collect();
    let evidence_items: Vec<EvidenceItem> = evidence
        .iter()
        .map(|e| EvidenceItem {
            title: e.node_title.clone(),
            path: e.source_path.clone(),
            content: e.content.clone(),
            doc_name: e.doc_name.clone(),
        })
        .collect();

    let content = if answer.is_empty() {
        evidence
            .iter()
            .map(|e| format!("## {}\n{}", e.node_title, e.content))
            .collect::<Vec<_>>()
            .join("\n\n---\n\n")
    } else {
        answer.to_string()
    };

    let evidence_count = evidence.len();
    let confidence = map_confidence(ConfidenceLevel::from_evidence(evidence_count, content.len()));

    QueryResultItem {
        doc_id: doc_id.to_string(),
        node_ids,
        content,
        score,
        evidence: evidence_items,
        metrics: Some(QueryMetrics {
            llm_calls: metrics.llm_calls,
            rounds_used: metrics.rounds_used,
            nodes_visited: metrics.nodes_visited,
            fast_path_hit: metrics.fast_path_hit,
            evidence_count,
            evidence_chars: metrics.evidence_chars,
        }),
        confidence,
    }
}

/// Build an empty result item (no evidence).
fn empty_item(doc_id: &str, answer: &str, score: f32) -> QueryResultItem {
    let content = if answer.is_empty() {
        String::new()
    } else {
        answer.to_string()
    };
    QueryResultItem {
        doc_id: doc_id.to_string(),
        node_ids: Vec::new(),
        content,
        score,
        evidence: Vec::new(),
        metrics: None,
        confidence: Confidence::Low,
    }
}

/// Map internal confidence to public API confidence.
fn map_confidence(level: ConfidenceLevel) -> Confidence {
    match level {
        ConfidenceLevel::High => Confidence::High,
        ConfidenceLevel::Medium => Confidence::Medium,
        ConfidenceLevel::Low => Confidence::Low,
    }
}
