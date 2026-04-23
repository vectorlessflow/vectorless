// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Result reranking — dedup + format.
//!
//! Post-processing pipeline that runs after the agent collects raw evidence:
//!
//! ```text
//! agent (collect evidence)
//!   → rerank::process()
//!     → dedup (quality filter + dedup)
//!     → format as answer (no LLM — return original text)
//!   → Output with final answer
//! ```
//!
//! This is a document retrieval engine. The answer IS the evidence.
//! No LLM synthesis, no rewriting. Find what you find, return what you find.

pub mod dedup;
pub mod types;

use tracing::info;

use vectorless_query::QueryIntent;
use types::{Evidence, RerankOutput};

/// Process agent output through the rerank pipeline.
///
/// Deduplicates evidence, then returns the original text as the answer.
/// No LLM calls — the Worker already retrieved the exact passages.
pub async fn process(
    _query: &str,
    evidence: &[Evidence],
    _multi_doc: bool,
    intent: QueryIntent,
    confidence: f32,
) -> vectorless_error::Result<RerankOutput> {
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

    let answer = match intent {
        QueryIntent::Navigational => format_locations(&deduped),
        _ => format_evidence_as_answer(&deduped),
    };

    info!(
        evidence = deduped.len(),
        answer_len = answer.len(),
        confidence,
        "Rerank complete"
    );

    Ok(RerankOutput {
        answer,
        llm_calls: 0,
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

/// Format collected evidence directly as the answer.
fn format_evidence_as_answer(evidence: &[Evidence]) -> String {
    evidence
        .iter()
        .map(|e| {
            let doc = e.doc_name.as_deref().unwrap_or("");
            if doc.is_empty() {
                format!("[{}]\n{}", e.node_title, e.content)
            } else {
                format!("[{} — {}]\n{}", e.node_title, doc, e.content)
            }
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}
