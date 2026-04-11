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

use super::decision::{InterventionPoint, PilotDecision, RankedCandidate, SearchDirection};
use crate::document::NodeId;

/// Parsed response from LLM.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmResponse {
    /// Ranked candidates with scores (FORK format).
    #[serde(default)]
    pub ranked_candidates: Vec<CandidateScore>,
    /// Entry points for START intervention (list of node titles).
    #[serde(default)]
    pub entry_points: Vec<String>,
    /// Best entry points (alternative START format from LLM).
    #[serde(default)]
    pub best_entry_points: Vec<EntryPoint>,
    /// Selected nodes (another alternative START format - list of titles).
    #[serde(default)]
    pub selected_nodes: Vec<String>,
    /// Selected node (singular - some LLMs return this format).
    #[serde(default)]
    pub selected_node: Option<String>,
    /// Recommended node (another singular format).
    #[serde(default)]
    pub recommended_node: Option<String>,
    /// Analysis wrapper (some LLMs nest response in "analysis" object).
    #[serde(default)]
    pub analysis: Option<AnalysisWrapper>,
    /// Recommended search direction.
    #[serde(default)]
    pub direction: DirectionResponse,
    /// Confidence level (0.0 - 1.0 or "high"/"medium"/"low").
    #[serde(default = "default_confidence", deserialize_with = "deserialize_confidence")]
    pub confidence: f32,
    /// Reasoning for the decision.
    #[serde(default)]
    pub reasoning: String,
}

/// Custom deserializer for confidence that accepts both float and string.
fn deserialize_confidence<'de, D>(deserializer: D) -> Result<f32, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = serde_json::Value::deserialize(deserializer)?;
    match value {
        serde_json::Value::Number(n) => {
            // Handle numeric value
            Ok(n.as_f64().unwrap_or(0.5) as f32)
        }
        serde_json::Value::String(s) => {
            // Handle string values like "high", "medium", "low"
            let lower = s.to_lowercase();
            let confidence = match lower.as_str() {
                "high" | "very high" | "strong" => 0.9,
                "medium" | "moderate" => 0.6,
                "low" | "weak" => 0.3,
                _ => 0.5, // default for unknown strings
            };
            Ok(confidence)
        }
        _ => Ok(0.5), // default for other types
    }
}

/// Analysis wrapper for nested LLM responses.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisWrapper {
    /// Query from analysis.
    #[serde(default)]
    pub query: Option<String>,
    /// Intent detected.
    #[serde(default)]
    pub intent: Option<String>,
    /// Selected node (singular).
    #[serde(default)]
    pub selected_node: Option<String>,
    /// Selected nodes (plural).
    #[serde(default)]
    pub selected_nodes: Vec<String>,
    /// Reasoning from analysis.
    #[serde(default)]
    pub reasoning: Option<String>,
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

/// Candidate info for title matching.
#[derive(Debug, Clone)]
pub struct CandidateInfo {
    /// Node ID.
    pub node_id: NodeId,
    /// Title of the node.
    pub title: String,
    /// Index in the candidates list.
    pub index: usize,
}

/// Entry point from START response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntryPoint {
    /// Node ID or index.
    #[serde(default)]
    pub node_id: Option<usize>,
    /// Index of the candidate.
    #[serde(default)]
    pub index: Option<usize>,
    /// Title of the entry point.
    #[serde(default)]
    pub title: Option<String>,
    /// Relevance score (may be 1-5 or 0.0-1.0).
    #[serde(default)]
    pub relevance_score: Option<f32>,
    /// Score (alternative field name).
    #[serde(default)]
    pub score: Option<f32>,
}

/// Top-3 candidate from LLM LOCatetop-3 response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Top3Candidate {
    /// Node ID from TO TO copy.
    pub node_id: usize,
    /// Relevance score (0.0-1.0).
    pub relevance_score: f32,
    /// Reason for the selection.
    pub reason: String,
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
                r"(?i)(go.?deeper|explore.?siblings|backtrack|found.?answer)",
            )
            .unwrap(),
        }
    }

    /// Parse LLM response into a PilotDecision.
    ///
    /// # Arguments
    ///
    /// * `response` - Raw LLM response text
    /// * `candidates` - Candidate info with NodeId, title, and index
    /// * `point` - The intervention point
    pub fn parse(
        &self,
        response: &str,
        candidates: &[CandidateInfo],
        point: InterventionPoint,
    ) -> PilotDecision {
        println!("[DEBUG] ResponseParser::parse() - candidates.len()={}", candidates.len());

        // Try JSON parse first
        if let Some(decision) = self.try_json_parse(response, candidates, point) {
            println!("[DEBUG] ResponseParser::parse() - JSON parse succeeded, ranked={}", decision.ranked_candidates.len());
            return decision;
        }
        println!("[DEBUG] ResponseParser::parse() - JSON parse failed, trying regex...");

        // Try regex extraction
        if let Some(decision) = self.try_regex_parse(response, candidates, point) {
            println!("[DEBUG] ResponseParser::parse() - Regex parse succeeded, ranked={}", decision.ranked_candidates.len());
            return decision;
        }
        println!("[DEBUG] ResponseParser::parse() - Regex parse failed, using default decision");

        // Return default decision
        self.default_decision(candidates, point)
    }

    /// Try to parse response as JSON.
    fn try_json_parse(
        &self,
        response: &str,
        candidates: &[CandidateInfo],
        point: InterventionPoint,
    ) -> Option<PilotDecision> {
        // First, try to extract JSON from code blocks
        let json_str = if let Some(caps) = self.json_block_regex.captures(response) {
            let extracted = caps.get(1)?.as_str().trim().to_string();
            println!("[DEBUG] ResponseParser::try_json_parse() - Found JSON in code block");
            extracted
        } else {
            // Try to find raw JSON object
            let start = response.find('{')?;
            let end = response.rfind('}')? + 1;
            let extracted = response[start..end].to_string();
            println!("[DEBUG] ResponseParser::try_json_parse() - Found raw JSON (no code block)");
            extracted
        };

        println!("[DEBUG] ResponseParser::try_json_parse() - Extracted JSON:\n{}", json_str);

        // Parse JSON
        let llm_response: LlmResponse = match serde_json::from_str::<LlmResponse>(&json_str) {
            Ok(r) => {
                println!("[DEBUG] ResponseParser::try_json_parse() - JSON parsed successfully");
                println!("[DEBUG] ResponseParser::try_json_parse() - ranked_candidates count: {}", r.ranked_candidates.len());
                r
            },
            Err(e) => {
                println!("[DEBUG] ResponseParser::try_json_parse() - JSON parse FAILED: {}", e);
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
        candidates: &[CandidateInfo],
        point: InterventionPoint,
    ) -> Option<PilotDecision> {
        // Extract confidence
        let confidence = self
            .confidence_regex
            .captures(response)
            .and_then(|caps| caps.get(1)?.as_str().parse::<f32>().ok())
            .unwrap_or(0.5)
            .clamp(0.0, 1.0);

        // Extract direction
        let direction = self
            .direction_regex
            .captures(response)
            .map(|caps| {
                let dir = caps.get(1)?.as_str().to_lowercase();
                match dir.as_str() {
                    d if d.contains("deeper") => Some(SearchDirection::GoDeeper {
                        reason: String::new(),
                    }),
                    d if d.contains("sibling") => Some(SearchDirection::ExploreSiblings {
                        recommended: vec![],
                    }),
                    d if d.contains("backtrack") => Some(SearchDirection::Backtrack {
                        reason: String::new(),
                        alternative_branches: vec![],
                    }),
                    d if d.contains("found") || d.contains("answer") => {
                        Some(SearchDirection::FoundAnswer { confidence })
                    }
                    _ => None,
                }
            })
            .flatten()
            .unwrap_or_else(|| SearchDirection::GoDeeper {
                reason: String::new(),
            });

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
    fn extract_ranked_candidates(
        &self,
        response: &str,
        candidates: &[CandidateInfo],
    ) -> Vec<RankedCandidate> {
        let mut ranked = Vec::new();

        // Pattern: "1. Candidate Name (score: 0.8)"
        let ranking_pattern =
            Regex::new(r"(\d+)[.\)]\s*(?:Candidate\s*)?(\d+)[\s:]+(?:score[:\s]*)?([0-9.]+)?")
                .unwrap();

        for caps in ranking_pattern.captures_iter(response) {
            if let Some(index_match) = caps.get(2) {
                if let Ok(index) = index_match.as_str().parse::<usize>() {
                    let score: f32 = caps
                        .get(3)
                        .and_then(|m| m.as_str().parse().ok())
                        .unwrap_or(0.5);

                    if index < candidates.len() {
                        ranked.push(RankedCandidate {
                            node_id: candidates[index].node_id,
                            score: score.clamp(0.0, 1.0),
                            reason: None,
                        });
                    }
                }
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
            if let Some(match_1) = caps.get(1) {
                if let Ok(idx) = match_1.as_str().parse::<usize>() {
                    if idx < candidates.len() && seen.insert(idx) {
                        ranked.push(RankedCandidate {
                            node_id: candidates[idx].node_id,
                            score: 1.0 - (ranked.len() as f32 * 0.1), // Decreasing scores
                            reason: None,
                        });
                    }
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
        mut llm_response: LlmResponse,
        candidates: &[CandidateInfo],
        point: InterventionPoint,
    ) -> PilotDecision {
        println!("[DEBUG] ResponseParser::llm_response_to_decision() - point={:?}", point);
        println!("[DEBUG] ResponseParser::llm_response_to_decision() - ranked_candidates.len()={}", llm_response.ranked_candidates.len());
        println!("[DEBUG] ResponseParser::llm_response_to_decision() - best_entry_points.len()={}", llm_response.best_entry_points.len());
        println!("[DEBUG] ResponseParser::llm_response_to_decision() - entry_points.len()={}", llm_response.entry_points.len());
        println!("[DEBUG] ResponseParser::llm_response_to_decision() - selected_nodes.len()={}", llm_response.selected_nodes.len());
        println!("[DEBUG] ResponseParser::llm_response_to_decision() - selected_node={:?}", llm_response.selected_node);
        println!("[DEBUG] ResponseParser::llm_response_to_decision() - analysis={:?}", llm_response.analysis.as_ref().map(|a| (&a.selected_node, &a.selected_nodes)));

        // Convert candidate scores to RankedCandidate
        let mut ranked_candidates: Vec<RankedCandidate> = llm_response
            .ranked_candidates
            .iter()
            .filter_map(|cs| {
                if cs.index < candidates.len() {
                    Some(RankedCandidate {
                        node_id: candidates[cs.index].node_id,
                        score: cs.score.clamp(0.0, 1.0),
                        reason: cs.reason.clone(),
                    })
                } else {
                    None
                }
            })
            .collect();

        // Handle START response format: best_entry_points, entry_points, or selected_nodes
        if ranked_candidates.is_empty() {
            // Try to convert best_entry_points (format: [{"node_id": 1, "title": "...", "relevance_score": 5}])
            for entry in &llm_response.best_entry_points {
                // Get index from either node_id or index field
                // node_id is 1-indexed from LLM, convert to 0-indexed
                let idx = if let Some(nid) = entry.node_id {
                    if nid > 0 { nid - 1 } else { nid }
                } else if let Some(idx) = entry.index {
                    idx
                } else {
                    continue; // Skip if no valid index
                };

                if idx < candidates.len() {
                    let score = entry.relevance_score
                        .or(entry.score)
                        .unwrap_or(0.5)
                        / 5.0; // Normalize 1-5 scale to 0.0-1.0
                    ranked_candidates.push(RankedCandidate {
                        node_id: candidates[idx].node_id,
                        score: score.clamp(0.0, 1.0),
                        reason: entry.title.clone(),
                    });
                    println!("[DEBUG] ResponseParser - converted best_entry_point[{}] to ranked_candidate (idx={}, score={:.2})",
                        idx, idx, score);
                }
            }

            // Try to convert selected_nodes (format: ["Project Documentation", "Overview"])
            // Match by title
            for selected_title in &llm_response.selected_nodes {
                for candidate in candidates {
                    if Self::titles_match(selected_title, &candidate.title) {
                        ranked_candidates.push(RankedCandidate {
                            node_id: candidate.node_id,
                            score: 0.9, // High score for title match
                            reason: Some(format!("Title match: {}", selected_title)),
                        });
                        println!("[DEBUG] ResponseParser - matched selected_node '{}' to candidate '{}' (index={})",
                            selected_title, candidate.title, candidate.index);
                        break; // Only match once per selected_node
                    }
                }
            }

            // Try to convert selected_node (singular - format: "Project Documentation")
            if let Some(ref single_node) = llm_response.selected_node {
                for candidate in candidates {
                    if Self::titles_match(single_node, &candidate.title) {
                        if !ranked_candidates.iter().any(|rc| rc.node_id == candidate.node_id) {
                            ranked_candidates.push(RankedCandidate {
                                node_id: candidate.node_id,
                                score: 0.9,
                                reason: Some(format!("Title match (singular): {}", single_node)),
                            });
                            println!("[DEBUG] ResponseParser - matched selected_node (singular) '{}' to candidate '{}' (index={})",
                                single_node, candidate.title, candidate.index);
                        }
                        break;
                    }
                }
            }

            // Try to convert recommended_node (another singular format)
            if let Some(ref recommended) = llm_response.recommended_node {
                for candidate in candidates {
                    if Self::titles_match(recommended, &candidate.title) {
                        if !ranked_candidates.iter().any(|rc| rc.node_id == candidate.node_id) {
                            ranked_candidates.push(RankedCandidate {
                                node_id: candidate.node_id,
                                score: 0.85,
                                reason: Some(format!("Recommended node: {}", recommended)),
                            });
                            println!("[DEBUG] ResponseParser - matched recommended_node '{}' to candidate '{}' (index={})",
                                recommended, candidate.title, candidate.index);
                        }
                        break;
                    }
                }
            }

            // Try to extract from analysis wrapper if present
            if let Some(ref analysis) = llm_response.analysis {
                // Check analysis.selected_nodes (plural array)
                for selected_title in &analysis.selected_nodes {
                    for candidate in candidates {
                        if Self::titles_match(selected_title, &candidate.title) {
                            if !ranked_candidates.iter().any(|rc| rc.node_id == candidate.node_id) {
                                ranked_candidates.push(RankedCandidate {
                                    node_id: candidate.node_id,
                                    score: 0.85,
                                    reason: Some(format!("Analysis selected_nodes: {}", selected_title)),
                                });
                                println!("[DEBUG] ResponseParser - matched analysis.selected_nodes '{}' to candidate '{}' (index={})",
                                    selected_title, candidate.title, candidate.index);
                            }
                            break;
                        }
                    }
                }

                // Check analysis.selected_node (singular)
                if let Some(ref single_node) = analysis.selected_node {
                    for candidate in candidates {
                        if Self::titles_match(single_node, &candidate.title) {
                            if !ranked_candidates.iter().any(|rc| rc.node_id == candidate.node_id) {
                                ranked_candidates.push(RankedCandidate {
                                    node_id: candidate.node_id,
                                    score: 0.85,
                                    reason: Some(format!("Analysis selected_node: {}", single_node)),
                                });
                                println!("[DEBUG] ResponseParser - matched analysis.selected_node (singular) '{}' to candidate '{}' (index={})",
                                    single_node, candidate.title, candidate.index);
                            }
                            break;
                        }
                    }
                }

                // Use analysis.reasoning if top-level reasoning is empty
                if llm_response.reasoning.is_empty() {
                    if let Some(ref r) = analysis.reasoning {
                        llm_response.reasoning = r.clone();
                    }
                }
            }

            // Try to convert entry_points (format: ["Node Title 1", "Node Title 2"])
            for entry_title in &llm_response.entry_points {
                for candidate in candidates {
                    if Self::titles_match(entry_title, &candidate.title) {
                        // Check if already added
                        if !ranked_candidates.iter().any(|rc| rc.node_id == candidate.node_id) {
                            ranked_candidates.push(RankedCandidate {
                                node_id: candidate.node_id,
                                score: 0.8, // Slightly lower score for entry_points
                                reason: Some(format!("Entry point: {}", entry_title)),
                            });
                            println!("[DEBUG] ResponseParser - matched entry_point '{}' to candidate '{}' (index={})",
                                entry_title, candidate.title, candidate.index);
                        }
                        break;
                    }
                }
            }
        }

        // Convert direction
        let direction = match llm_response.direction {
            DirectionResponse::GoDeeper => SearchDirection::GoDeeper {
                reason: llm_response.reasoning.clone(),
            },
            DirectionResponse::ExploreSiblings => SearchDirection::ExploreSiblings {
                recommended: ranked_candidates
                    .iter()
                    .take(3)
                    .map(|c| c.node_id)
                    .collect(),
            },
            DirectionResponse::Backtrack => SearchDirection::Backtrack {
                reason: llm_response.reasoning.clone(),
                alternative_branches: ranked_candidates
                    .iter()
                    .take(3)
                    .map(|c| c.node_id)
                    .collect(),
            },
            DirectionResponse::FoundAnswer => SearchDirection::FoundAnswer {
                confidence: llm_response.confidence,
            },
        };

        println!("[DEBUG] ResponseParser::llm_response_to_decision() - final ranked_candidates.len()={}", ranked_candidates.len());

        PilotDecision {
            ranked_candidates,
            direction,
            confidence: llm_response.confidence.clamp(0.0, 1.0),
            reasoning: llm_response.reasoning,
            intervention_point: point,
        }
    }

    /// Check if two titles match (fuzzy matching).
    fn titles_match(llm_title: &str, candidate_title: &str) -> bool {
        let llm_lower = llm_title.to_lowercase().trim().to_string();
        let candidate_lower = candidate_title.to_lowercase().trim().to_string();

        // Exact match
        if llm_lower == candidate_lower {
            return true;
        }

        // Contains match
        if llm_lower.contains(&candidate_lower) || candidate_lower.contains(&llm_lower) {
            return true;
        }

        // Word overlap match (at least 50% of words match)
        let llm_words: std::collections::HashSet<&str> = llm_lower.split_whitespace().collect();
        let candidate_words: std::collections::HashSet<&str> = candidate_lower.split_whitespace().collect();
        let overlap = llm_words.intersection(&candidate_words).count();
        let min_words = llm_words.len().min(candidate_words.len());
        if min_words > 0 && overlap as f32 / min_words as f32 >= 0.5 {
            return true;
        }

        false
    }

    /// Create a default decision when parsing fails.
    fn default_decision(&self, candidates: &[CandidateInfo], point: InterventionPoint) -> PilotDecision {
        // Score candidates uniformly
        let ranked: Vec<RankedCandidate> = candidates
            .iter()
            .enumerate()
            .map(|(i, c)| RankedCandidate {
                node_id: c.node_id,
                score: 1.0 / (i + 1) as f32, // Decreasing scores
                reason: None,
            })
            .collect();

        PilotDecision {
            ranked_candidates: ranked,
            direction: SearchDirection::GoDeeper {
                reason: String::new(),
            },
            confidence: 0.0,
            reasoning: "Default decision (parsing failed)".to_string(),
            intervention_point: point,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use indextree::Arena;

    fn create_test_node_ids(count: usize) -> Vec<NodeId> {
        let mut arena = Arena::new();
        let mut ids = Vec::new();
        for i in 0..count {
            let node = crate::document::TreeNode {
                title: format!("Node {}", i),
                structure: String::new(),
                content: String::new(),
                summary: String::new(),
                depth: 0,
                start_index: 1,
                end_index: 1,
                start_page: None,
                end_page: None,
                node_id: None,
                physical_index: None,
                token_count: None,
                references: Vec::new(),
            };
            ids.push(NodeId(arena.new_node(node)));
        }
        ids
    }
}
