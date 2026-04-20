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
//!     → intent-driven synthesis/fusion
//!   → Output with final answer
//! ```
//!
//! Synthesis strategy is driven by [`QueryIntent`] from query understanding.
//! The agent only collects evidence; all organizing, ranking, and answer
//! generation happens here.

pub mod dedup;
pub mod fusion;
pub mod scorer;
pub mod synthesis;
pub mod types;

use tracing::info;

use crate::agent::{Evidence, Output};
use crate::llm::LlmClient;
use crate::query::QueryIntent;
use types::{ConfidenceLevel, RerankOutput};

/// Process agent output through the rerank pipeline.
///
/// Takes raw agent output (evidence without answer) and produces
/// a final answer through dedup → score → intent-driven synthesis.
///
/// Returns [`Result<RerankOutput>`]. Propagates LLM errors — no silent fallback.
pub async fn process(
    query: &str,
    evidence: &[Evidence],
    llm: &LlmClient,
    multi_doc: bool,
    sub_results: &[Output],
    intent: QueryIntent,
) -> crate::error::Result<RerankOutput> {
    // Step 1: Deduplicate
    let deduped = dedup::dedup(evidence);
    if deduped.is_empty() {
        info!("No evidence after dedup");
        return Ok(RerankOutput {
            answer: String::new(),
            score: 0.0,
            llm_calls: 0,
            confidence: ConfidenceLevel::Low,
        });
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
        intent = %intent,
        "Evidence after dedup + scoring"
    );

    // Step 3: Intent-driven synthesis (No thought, no answer).
    let (answer, llm_calls) = match intent {
        QueryIntent::Navigational => {
            // Navigational: format locations, no deep synthesis needed
            (format_locations(&sorted_evidence), 0)
        }
        QueryIntent::Analytical if multi_doc && sub_results.len() > 1 => {
            // Analytical multi-doc: fuse across sub-results
            let sub_refs: Vec<&Output> = sub_results.iter().collect();
            fusion::fuse(query, &sub_refs, llm).await?
        }
        _ => {
            // Factual, Summary, Analytical single-doc: synthesis
            synthesis::synthesize(query, &sorted_evidence, llm).await?
        }
    };

    let confidence = ConfidenceLevel::from_evidence(sorted_evidence.len(), answer.len());
    info!(
        evidence = sorted_evidence.len(),
        answer_len = answer.len(),
        confidence = ?confidence,
        "Rerank complete"
    );

    Ok(RerankOutput {
        answer,
        score: top_score,
        llm_calls,
        confidence,
    })
}

/// Format evidence as a location listing for navigational queries.
fn format_locations(evidence: &[Evidence]) -> String {
    if evidence.is_empty() {
        return "No matching locations found.".to_string();
    }
    let mut result = "Found at:\n".to_string();
    for e in evidence {
        let doc = e.doc_name.as_deref().unwrap_or("unknown");
        result.push_str(&format!(
            "- **{}** in {} at {}\n",
            e.node_title, doc, e.source_path
        ));
    }
    result
}
