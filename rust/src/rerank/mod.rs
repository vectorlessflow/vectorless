// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Result reranking and answer synthesis.
//!
//! Post-processing pipeline that runs after the agent collects raw evidence:
//!
//! ```text
//! agent (collect evidence)
//!   → rerank::process()
//!     → dedup (quality filter + dedup)
//!     → scorer (BM25 relevance ranking)
//!     → fusion (multi-doc, optional) OR synthesis (single-doc)
//!   → Output with final answer
//! ```
//!
//! This is the unified post-processing path. The agent only collects evidence;
//! all organizing, ranking, and answer generation happens here.

pub mod dedup;
pub mod fusion;
pub mod scorer;
pub mod synthesis;
pub mod types;

use tracing::info;

use crate::agent::{Evidence, Output};
use crate::llm::LlmClient;
use types::{ConfidenceLevel, RerankOutput};

/// Process agent output through the rerank pipeline.
///
/// Takes raw agent output (evidence without answer) and produces
/// a final answer through dedup → score → fuse/synthesize.
///
/// Returns [`RerankOutput`] with answer, score, confidence, and LLM call count.
pub async fn process(
    query: &str,
    evidence: &[Evidence],
    enable_synthesis: bool,
    llm: &LlmClient,
    multi_doc: bool,
    sub_results: &[Output],
) -> RerankOutput {
    // Step 1: Deduplicate
    let deduped = dedup::dedup(evidence);
    if deduped.is_empty() {
        info!("No evidence after dedup");
        return RerankOutput {
            answer: String::new(),
            score: 0.0,
            llm_calls: 0,
            confidence: ConfidenceLevel::Low,
        };
    }

    // Step 2: Score and sort by relevance
    let scored = scorer::rank(query, &deduped);
    let top_score = scored.first().map(|(_, s)| *s).unwrap_or(0.0);
    let sorted_evidence: Vec<Evidence> = scored
        .iter()
        .map(|(idx, _)| deduped[*idx].clone())
        .collect();

    info!(
        evidence = sorted_evidence.len(),
        top_score,
        "Evidence after dedup + scoring"
    );

    // Step 3: Synthesize answer
    if !enable_synthesis {
        return RerankOutput {
            answer: synthesis::format_evidence_as_answer(&sorted_evidence),
            score: top_score,
            llm_calls: 0,
            confidence: ConfidenceLevel::from_evidence(sorted_evidence.len(), 0),
        };
    }

    let (answer, llm_calls) = if multi_doc && sub_results.len() > 1 {
        // Multi-doc: fuse across sub-results
        let sub_refs: Vec<&Output> = sub_results.iter().collect();
        fusion::fuse(query, &sub_refs, llm).await
    } else {
        // Single doc: simple synthesis
        synthesis::synthesize(query, &sorted_evidence, llm).await
    };

    let confidence = ConfidenceLevel::from_evidence(sorted_evidence.len(), answer.len());
    info!(
        evidence = sorted_evidence.len(),
        answer_len = answer.len(),
        confidence = ?confidence,
        "Rerank complete"
    );

    RerankOutput {
        answer,
        score: top_score,
        llm_calls,
        confidence,
    }
}
