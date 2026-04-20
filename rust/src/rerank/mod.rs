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
//!     → intent-driven synthesis/fusion
//!   → Output with final answer
//! ```
//!
//! Synthesis strategy is driven by [`QueryIntent`] from query understanding.
//! The agent only collects evidence; all organizing and answer generation
//! happens here. Confidence is derived from the LLM evaluate() result
//! in the Orchestrator's supervisor loop — not from heuristic scoring.

pub mod dedup;
pub mod fusion;
pub mod synthesis;
pub mod types;

use tracing::info;

use crate::agent::{Evidence, Output};
use crate::llm::LlmClient;
use crate::query::QueryIntent;
use types::RerankOutput;

/// Process agent output through the rerank pipeline.
///
/// Takes raw agent output (evidence without answer) and produces
/// a final answer through dedup → intent-driven synthesis.
///
/// Confidence is passed from the Orchestrator (derived from LLM evaluate).
/// Returns [`Result<RerankOutput>`]. Propagates LLM errors — no silent fallback.
pub async fn process(
    query: &str,
    evidence: &[Evidence],
    llm: &LlmClient,
    multi_doc: bool,
    sub_results: &[Output],
    intent: QueryIntent,
    confidence: f32,
) -> crate::error::Result<RerankOutput> {
    // Step 1: Deduplicate
    let deduped = dedup::dedup(evidence);
    if deduped.is_empty() {
        info!("No evidence after dedup");
        return Ok(RerankOutput {
            answer: String::new(),
            llm_calls: 0,
            confidence: 0.0,
        });
    }

    info!(
        evidence = deduped.len(),
        intent = %intent,
        "Evidence after dedup"
    );

    // Step 2: Intent-driven synthesis (No thought, no answer).
    let (answer, llm_calls) = match intent {
        QueryIntent::Navigational => {
            // Navigational: format locations, no deep synthesis needed
            (format_locations(&deduped), 0)
        }
        QueryIntent::Analytical if multi_doc && sub_results.len() > 1 => {
            // Analytical multi-doc: fuse across sub-results
            let sub_refs: Vec<&Output> = sub_results.iter().collect();
            fusion::fuse(query, &sub_refs, llm).await?
        }
        _ => {
            // Factual, Summary, Analytical single-doc: synthesis
            synthesis::synthesize(query, &deduped, llm).await?
        }
    };

    info!(
        evidence = deduped.len(),
        answer_len = answer.len(),
        confidence,
        "Rerank complete"
    );

    Ok(RerankOutput {
        answer,
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
