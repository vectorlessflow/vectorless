// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Cross-document evidence fusion.

use tracing::info;

use crate::agent::Output;
use crate::llm::LlmClient;

/// Summary of a Worker result for the fusion prompt.
pub struct WorkerSummary<'a> {
    pub doc_name: &'a str,
    pub evidence_count: usize,
    pub evidence_text: &'a str,
    pub answer: &'a str,
}

/// Parameters for the multi-doc fusion prompt.
pub struct FusionParams<'a> {
    pub query: &'a str,
    pub sub_results: &'a [WorkerSummary<'a>],
}

/// Build the cross-document fusion prompt.
pub fn fusion_prompt(params: &FusionParams) -> (String, String) {
    let query = params.query;

    let system =
        "You are a multi-document analysis assistant. You are given evidence independently \
         collected from multiple documents. Your job is to integrate this evidence to answer \
         the user's question.

Requirements:
- Mark the source document for each piece of information.
- If different documents have conflicting data, point out the discrepancy.
- If units or measurement criteria differ, explain the difference.
- If evidence is missing for some aspect, state it clearly."
            .to_string();

    let mut evidence_sections = String::new();
    for result in params.sub_results {
        evidence_sections.push_str(&format!(
            "## Document: {} ({} evidence items)\n{}\n",
            result.doc_name, result.evidence_count, result.evidence_text
        ));
        if !result.answer.is_empty() {
            evidence_sections.push_str(&format!("Sub-answer: {}\n", result.answer));
        }
        evidence_sections.push('\n');
    }

    let user = format!(
        "User question: {query}\n\n\
         Collected evidence:\n\
         {evidence_sections}\n\
         Integrated analysis:"
    );

    (system, user)
}

/// Fuse multiple Worker results into a single answer via LLM.
///
/// Returns (answer, llm_calls). Propagates LLM errors — no silent fallback.
pub async fn fuse(
    query: &str,
    sub_results: &[&Output],
    llm: &LlmClient,
) -> crate::error::Result<(String, u32)> {
    // Build intermediate summaries from sub-results
    struct SubResultData {
        doc_name: String,
        evidence_count: usize,
        evidence_text: String,
        answer: String,
    }

    let summaries: Vec<SubResultData> = sub_results
        .iter()
        .map(|result| {
            let doc_name = result
                .evidence
                .first()
                .and_then(|e| e.doc_name.clone())
                .unwrap_or_else(|| "unknown".to_string());
            let evidence_text = result
                .evidence
                .iter()
                .map(|e| format!("[{}] {}", e.node_title, e.content))
                .collect::<Vec<_>>()
                .join("\n");
            SubResultData {
                evidence_count: result.evidence.len(),
                doc_name,
                evidence_text,
                answer: result.answer.clone(),
            }
        })
        .collect();

    let summary_refs: Vec<WorkerSummary<'_>> = summaries
        .iter()
        .map(|s| WorkerSummary {
            doc_name: &s.doc_name,
            evidence_count: s.evidence_count,
            evidence_text: &s.evidence_text,
            answer: &s.answer,
        })
        .collect();

    let (system, user) = fusion_prompt(&FusionParams {
        query,
        sub_results: &summary_refs,
    });

    match llm.complete(&system, &user).await {
        Ok(a) => {
            let answer = a.trim().to_string();
            if answer.is_empty() {
                return Err(crate::error::Error::LlmReasoning {
                    stage: "fusion".to_string(),
                    detail: "LLM returned empty answer".to_string(),
                });
            }
            info!(answer_len = answer.len(), "Fusion synthesis complete");
            Ok((answer, 1))
        }
        Err(e) => Err(crate::error::Error::LlmReasoning {
            stage: "fusion".to_string(),
            detail: format!("LLM call failed: {}", e),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fusion_prompt() {
        let summaries = [WorkerSummary {
            doc_name: "doc1",
            evidence_count: 2,
            evidence_text: "[A] content A\n[B] content B",
            answer: "sub answer",
        }];
        let (system, user) = fusion_prompt(&FusionParams {
            query: "test query",
            sub_results: &summaries,
        });
        assert!(system.contains("multi-document"));
        assert!(user.contains("test query"));
        assert!(user.contains("doc1"));
    }
}
