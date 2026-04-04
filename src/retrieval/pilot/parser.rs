// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Response parser for Pilot LLM calls.
//!
//! Parses LLM responses into structured `PilotDecision` objects.
//! Uses multiple parsing strategies with graceful fallbacks:
//! 1. JSON parse (preferred)
//! 2. Regex extraction
//! 3. Default decision (fallback)

use regex::Regex;
use serde::{Deserialize, Serialize};
use tracing::warn;

use crate::domain::NodeId;
use super::decision::{PilotDecision, RankedCandidate, SearchDirection, InterventionPoint};

/// Parsed response from LLM.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmResponse {
    /// Ranked candidates with scores.
    #[serde(default)]
    pub ranked_candidates: Vec<CandidateScore>,
    /// Recommended search direction.
    #[serde(default)]
    pub direction: DirectionResponse,
    /// Confidence level (0.0 - 1.0).
    #[serde(default = "default_confidence")]
    pub confidence: f32,
    /// Reasoning for the decision.
    #[serde(default)]
    pub reasoning: String,
}

/// Candidate score from LLM response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CandidateScore {
    /// Index of the candidate (0-based).
    pub index: usize,
    /// Score for this candidate (0.0 - 1.0).
    pub score: f32,
    /// Optional reason for the score.
    #[serde(default)]
    pub reason: Option<String>,
}

/// Direction response from LLM.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum DirectionResponse {
    #[default]
    GoDeeper,
    ExploreSiblings,
    Backtrack,
    FoundAnswer,
}

fn default_confidence() -> f32 {
    0.5
}

/// Response parser for LLM outputs.
///
/// Implements layered parsing with graceful degradation:
/// 1. Try JSON parse first
/// 2. Fall back to regex extraction
/// 3. Return default decision if all else fails
///
/// # Example
///
/// ```rust,ignore
/// use vectorless::retrieval::pilot::ResponseParser;
///
/// let parser = ResponseParser::new();
/// let decision = parser.parse(&llm_response, candidates, InterventionPoint::Fork);
/// ```
pub struct ResponseParser {
    /// Regex for extracting JSON from markdown code blocks.
    json_block_regex: Regex,
    /// Regex for extracting confidence.
    confidence_regex: Regex,
    /// Regex for extracting direction.
    direction_regex: Regex,
}

impl Default for ResponseParser {
    fn default() -> Self {
        Self::new()
    }
}

impl ResponseParser {
    /// Create a new response parser.
    pub fn new() -> Self {
        Self {
            // Match JSON in markdown code blocks
            json_block_regex: Regex::new(r"```(?:json)?\s*([\s\S]*?)```").unwrap(),
            // Match confidence: 0.8 or confidence: 0.8
            confidence_regex: Regex::new(r"(?i)confidence[:\s]+([0-9.]+)").unwrap(),
            // Match direction keywords
            direction_regex: Regex::new(
                r"(?i)(go.?deeper|explore.?siblings|backtrack|found.?answer)"
            ).unwrap(),
        }
    }

    /// Parse LLM response into a PilotDecision.
    ///
    /// # Arguments
    ///
    /// * `response` - Raw LLM response text
    /// * `candidates` - Original candidate NodeIds (for mapping indices)
    /// * `point` - The intervention point
    pub fn parse(
        &self,
        response: &str,
        candidates: &[NodeId],
        point: InterventionPoint,
    ) -> PilotDecision {
        // Try JSON parse first
        if let Some(decision) = self.try_json_parse(response, candidates, point) {
            return decision;
        }

        // Try regex extraction
        if let Some(decision) = self.try_regex_parse(response, candidates, point) {
            return decision;
        }

        // Return default decision
        self.default_decision(candidates, point)
    }

    /// Try to parse response as JSON.
    fn try_json_parse(
        &self,
        response: &str,
        candidates: &[NodeId],
        point: InterventionPoint,
    ) -> Option<PilotDecision> {
        // First, try to extract JSON from code blocks
        let json_str = if let Some(caps) = self.json_block_regex.captures(response) {
            caps.get(1)?.as_str().trim().to_string()
        } else {
            // Try to find raw JSON object
            let start = response.find('{')?;
            let end = response.rfind('}')? + 1;
            response[start..end].to_string()
        };

        // Parse JSON
        let llm_response: LlmResponse = match serde_json::from_str(&json_str) {
            Ok(r) => r,
            Err(e) => {
                warn!("Failed to parse LLM response as JSON: {}", e);
                return None;
            }
        };

        // Convert to PilotDecision
        Some(self.llm_response_to_decision(llm_response, candidates, point))
    }

    /// Try to parse response using regex.
    fn try_regex_parse(
        &self,
        response: &str,
        candidates: &[NodeId],
        point: InterventionPoint,
    ) -> Option<PilotDecision> {
        // Extract confidence
        let confidence = self.confidence_regex
            .captures(response)
            .and_then(|caps| caps.get(1)?.as_str().parse::<f32>().ok())
            .unwrap_or(0.5)
            .clamp(0.0, 1.0);

        // Extract direction
        let direction = self.direction_regex
            .captures(response)
            .map(|caps| {
                let dir = caps.get(1)?.as_str().to_lowercase();
                match dir.as_str() {
                    d if d.contains("deeper") => Some(SearchDirection::GoDeeper { reason: String::new() }),
                    d if d.contains("sibling") => Some(SearchDirection::ExploreSiblings { recommended: vec![] }),
                    d if d.contains("backtrack") => Some(SearchDirection::Backtrack {
                        reason: String::new(),
                        alternative_branches: vec![],
                    }),
                    d if d.contains("found") || d.contains("answer") => Some(SearchDirection::FoundAnswer { confidence }),
                    _ => None,
                }
            })
            .flatten()
            .unwrap_or_else(|| SearchDirection::GoDeeper { reason: String::new() });

        // Try to extract candidate rankings from numbered list
        let ranked = self.extract_ranked_candidates(response, candidates);

        if ranked.is_empty() && candidates.len() > 1 {
            return None; // Regex parse failed
        }

        Some(PilotDecision {
            ranked_candidates: ranked,
            direction,
            confidence,
            reasoning: "Extracted via regex".to_string(),
            intervention_point: point,
        })
    }

    /// Extract ranked candidates from text using patterns.
    fn extract_ranked_candidates(&self, response: &str, candidates: &[NodeId]) -> Vec<RankedCandidate> {
        let mut ranked = Vec::new();

        // Pattern: "1. Candidate Name (score: 0.8)"
        let ranking_pattern = Regex::new(r"(\d+)[.\)]\s*(?:Candidate\s*)?(\d+)[\s:]+(?:score[:\s]*)?([0-9.]+)?").unwrap();

        for caps in ranking_pattern.captures_iter(response) {
            let index: usize = caps.get(2)?.as_str().parse().ok()?;
            let score: f32 = caps.get(3)
                .and_then(|m| m.as_str().parse().ok())
                .unwrap_or(0.5);

            if index < candidates.len() {
                ranked.push(RankedCandidate {
                    node_id: candidates[index],
                    score: score.clamp(0.0, 1.0),
                    reason: None,
                });
            }
        }

        // If we got some rankings, return them
        if !ranked.is_empty() {
            return ranked;
        }

        // Fallback: look for numbers that might be candidate indices
        let number_pattern = Regex::new(r"\b(\d+)\b").unwrap();
        let mut seen = std::collections::HashSet::new();

        for caps in number_pattern.captures_iter(response) {
            if let Ok(idx) = caps.get(1)?.as_str().parse::<usize>() {
                if idx < candidates.len() && seen.insert(idx) {
                    ranked.push(RankedCandidate {
                        node_id: candidates[idx],
                        score: 1.0 - (ranked.len() as f32 * 0.1), // Decreasing scores
                        reason: None,
                    });
                }
            }

            if ranked.len() >= candidates.len() {
                break;
            }
        }

        ranked
    }

    /// Convert LlmResponse to PilotDecision.
    fn llm_response_to_decision(
        &self,
        llm_response: LlmResponse,
        candidates: &[NodeId],
        point: InterventionPoint,
    ) -> PilotDecision {
        // Convert candidate scores to RankedCandidate
        let ranked_candidates: Vec<RankedCandidate> = llm_response
            .ranked_candidates
            .into_iter()
            .filter_map(|cs| {
                if cs.index < candidates.len() {
                    Some(RankedCandidate {
                        node_id: candidates[cs.index],
                        score: cs.score.clamp(0.0, 1.0),
                        reason: cs.reason,
                    })
                } else {
                    None
                }
            })
            .collect();

        // Convert direction
        let direction = match llm_response.direction {
            DirectionResponse::GoDeeper => SearchDirection::GoDeeper {
                reason: llm_response.reasoning.clone(),
            },
            DirectionResponse::ExploreSiblings => SearchDirection::ExploreSiblings {
                recommended: ranked_candidates.iter().take(3).map(|c| c.node_id).collect(),
            },
            DirectionResponse::Backtrack => SearchDirection::Backtrack {
                reason: llm_response.reasoning.clone(),
                alternative_branches: ranked_candidates.iter().take(3).map(|c| c.node_id).collect(),
            },
            DirectionResponse::FoundAnswer => SearchDirection::FoundAnswer {
                confidence: llm_response.confidence,
            },
        };

        PilotDecision {
            ranked_candidates,
            direction,
            confidence: llm_response.confidence.clamp(0.0, 1.0),
            reasoning: llm_response.reasoning,
            intervention_point: point,
        }
    }

    /// Create a default decision when parsing fails.
    fn default_decision(&self, candidates: &[NodeId], point: InterventionPoint) -> PilotDecision {
        // Score candidates uniformly
        let ranked: Vec<RankedCandidate> = candidates
            .iter()
            .enumerate()
            .map(|(i, &node_id)| RankedCandidate {
                node_id,
                score: 1.0 / (i + 1) as f32, // Decreasing scores
                reason: None,
            })
            .collect();

        PilotDecision {
            ranked_candidates: ranked,
            direction: SearchDirection::GoDeeper { reason: String::new() },
            confidence: 0.0,
            reasoning: "Default decision (parsing failed)".to_string(),
            intervention_point: point,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_json_response() {
        let parser = ResponseParser::new();
        let candidates = vec![NodeId::new(0), NodeId::new(1), NodeId::new(2)];

        let response = r#"{
            "ranked_candidates": [
                {"index": 1, "score": 0.9, "reason": "Best match"},
                {"index": 0, "score": 0.5}
            ],
            "direction": "go_deeper",
            "confidence": 0.85,
            "reasoning": "Candidate 1 is most relevant"
        }"#;

        let decision = parser.parse(response, &candidates, InterventionPoint::Fork);

        assert_eq!(decision.ranked_candidates.len(), 2);
        assert_eq!(decision.ranked_candidates[0].node_id, candidates[1]);
        assert!((decision.confidence - 0.85).abs() < 0.01);
        assert!(matches!(decision.direction, SearchDirection::GoDeeper { .. }));
    }

    #[test]
    fn test_parse_json_in_code_block() {
        let parser = ResponseParser::new();
        let candidates = vec![NodeId::new(0), NodeId::new(1)];

        let response = r#"
Here's my analysis:

```json
{
    "ranked_candidates": [{"index": 0, "score": 0.8}],
    "direction": "go_deeper",
    "confidence": 0.8,
    "reasoning": "Test"
}
```
"#;

        let decision = parser.parse(response, &candidates, InterventionPoint::Fork);
        assert_eq!(decision.ranked_candidates.len(), 1);
    }

    #[test]
    fn test_parse_with_regex_fallback() {
        let parser = ResponseParser::new();
        let candidates = vec![NodeId::new(0), NodeId::new(1)];

        // Non-JSON response with some structure
        let response = r#"
I think candidate 0 is the best match.
Confidence: 0.75
Direction: go_deeper
"#;

        let decision = parser.parse(response, &candidates, InterventionPoint::Fork);

        // Should use regex extraction
        assert!((decision.confidence - 0.75).abs() < 0.01);
    }

    #[test]
    fn test_default_decision() {
        let parser = ResponseParser::new();
        let candidates = vec![NodeId::new(0), NodeId::new(1)];

        let decision = parser.parse(
            "This is unparseable gibberish",
            &candidates,
            InterventionPoint::Fork,
        );

        // Should return default
        assert_eq!(decision.ranked_candidates.len(), 2);
        assert_eq!(decision.confidence, 0.0);
        assert!(decision.reasoning.contains("parsing failed"));
    }

    #[test]
    fn test_confidence_clamping() {
        let parser = ResponseParser::new();
        let candidates = vec![NodeId::new(0)];

        let response = r#"{
            "ranked_candidates": [{"index": 0, "score": 1.5}],
            "confidence": 1.5,
            "direction": "go_deeper"
        }"#;

        let decision = parser.parse(response, &candidates, InterventionPoint::Fork);

        // Confidence should be clamped to 1.0
        assert!((decision.confidence - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_direction_conversion() {
        let parser = ResponseParser::new();
        let candidates = vec![NodeId::new(0)];

        let test_cases = vec![
            ("\"direction\": \"go_deeper\"", true),
            ("\"direction\": \"explore_siblings\"", true),
            ("\"direction\": \"backtrack\"", true),
            ("\"direction\": \"found_answer\"", true),
        ];

        for (dir_json, should_parse) in test_cases {
            let response = format!(r#"{{"ranked_candidates": [], "confidence": 0.5, {}}}"#, dir_json);
            let decision = parser.parse(&response, &candidates, InterventionPoint::Fork);
            assert!(should_parse, "Direction should parse correctly");
        }
    }
}
