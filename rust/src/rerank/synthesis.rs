// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Answer synthesis — generate the final answer from collected evidence.

use tracing::info;

use crate::agent::Evidence;
use crate::llm::LlmClient;

/// Maximum total characters for evidence in the synthesis prompt.
const SYNTHESIS_EVIDENCE_CAP: usize = 10000;

/// Parameters for the answer synthesis prompt.
pub struct SynthesisParams<'a> {
    pub query: &'a str,
    pub evidence_text: &'a str,
    pub missing_info: &'a str,
}

/// Build the answer synthesis prompt.
pub fn answer_synthesis_prompt(params: &SynthesisParams) -> (String, String) {
    let query = params.query;
    let evidence_text = params.evidence_text;

    let system =
        "You are an expert analyst. Based on the provided evidence, directly answer the user's \
         question. Cite the source section for each piece of information you use. \
         If the evidence is insufficient to fully answer the question, clearly state what is known \
         and what is missing."
            .to_string();

    let missing_section = if params.missing_info.is_empty() {
        String::new()
    } else {
        format!(
            "\nNote: The following information may be missing: {}",
            params.missing_info
        )
    };

    let user = format!(
        "User question: {query}\n\n\
         Evidence:\n\
         {evidence_text}{missing_section}\n\n\
         Answer:"
    );

    (system, user)
}

/// Synthesize an answer from evidence using LLM.
///
/// Returns (answer, llm_calls). Propagates LLM errors — no silent fallback.
pub async fn synthesize(
    query: &str,
    evidence: &[Evidence],
    llm: &LlmClient,
) -> crate::error::Result<(String, u32)> {
    let evidence_text = format_evidence_for_synthesis(evidence);
    let (system, user) = answer_synthesis_prompt(&SynthesisParams {
        query,
        evidence_text: &evidence_text,
        missing_info: "",
    });

    match llm.complete(&system, &user).await {
        Ok(a) => {
            let answer = a.trim().to_string();
            if answer.is_empty() {
                return Err(crate::error::Error::LlmReasoning {
                    stage: "synthesis".to_string(),
                    detail: "LLM returned empty answer".to_string(),
                });
            }
            info!(answer_len = answer.len(), "Synthesis complete");
            Ok((answer, 1))
        }
        Err(e) => Err(crate::error::Error::LlmReasoning {
            stage: "synthesis".to_string(),
            detail: format!("LLM call failed: {}", e),
        }),
    }
}

/// Format evidence for the synthesis prompt, with a total character cap.
pub fn format_evidence_for_synthesis(evidence: &[Evidence]) -> String {
    let mut result = String::new();
    for e in evidence {
        let doc = e.doc_name.as_deref().unwrap_or("unknown");
        let item = format!(
            "[{}] ({} at {})\n{}",
            e.node_title, doc, e.source_path, e.content
        );
        if result.len() + item.len() + 2 > SYNTHESIS_EVIDENCE_CAP {
            let remaining = SYNTHESIS_EVIDENCE_CAP.saturating_sub(result.len());
            if remaining > 50 {
                result.push_str(&format!(
                    "[{}] ({} at {})\n{}...[truncated]\n",
                    e.node_title,
                    doc,
                    e.source_path,
                    &e.content[..remaining.min(e.content.len())]
                ));
            }
            let remaining_count = evidence.len()
                - evidence
                    .iter()
                    .position(|x| x.node_title == e.node_title)
                    .unwrap_or(0)
                - 1;
            if remaining_count > 0 {
                result.push_str(&format!(
                    "\n... and {} more evidence items truncated to fit budget.\n",
                    remaining_count
                ));
            }
            break;
        }
        result.push_str(&item);
        result.push_str("\n\n");
    }
    result
}

/// Format evidence as a simple answer (fallback when synthesis is disabled or fails).
pub fn format_evidence_as_answer(evidence: &[Evidence]) -> String {
    evidence
        .iter()
        .map(|e| {
            let doc = e.doc_name.as_deref().unwrap_or("unknown");
            format!(
                "**{}** (from {} at {}):\n{}",
                e.node_title, doc, e.source_path, e.content
            )
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_evidence(title: &str, content: &str) -> Evidence {
        Evidence {
            source_path: format!("root/{}", title),
            node_title: title.to_string(),
            content: content.to_string(),
            doc_name: Some("my_doc".to_string()),
        }
    }

    #[test]
    fn test_format_evidence_for_synthesis() {
        let evidence = vec![make_evidence("A", "the answer")];
        let formatted = format_evidence_for_synthesis(&evidence);
        assert!(formatted.contains("[A]"));
        assert!(formatted.contains("my_doc"));
        assert!(formatted.contains("the answer"));
    }

    #[test]
    fn test_format_evidence_as_answer() {
        let evidence = vec![make_evidence("Y", "y content")];
        let formatted = format_evidence_as_answer(&evidence);
        assert!(formatted.contains("**Y**"));
        assert!(formatted.contains("my_doc"));
    }

    #[test]
    fn test_format_evidence_truncation() {
        let evidence: Vec<Evidence> = (0..100)
            .map(|i| make_evidence(&format!("Node {}", i), &"x".repeat(500)))
            .collect();
        let formatted = format_evidence_for_synthesis(&evidence);
        assert!(formatted.len() <= SYNTHESIS_EVIDENCE_CAP + 200); // some slack for truncation text
        assert!(formatted.contains("truncated"));
    }
}
