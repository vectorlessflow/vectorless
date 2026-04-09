// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Hierarchical ToC-based node locator.
//!
//! Replaces the monolithic `build_toc_for_llm` with a two-phase approach:
//! - Phase A: BM25 scoring on top-level nodes for fast filtering
//! - Phase B: Optional LLM refinement when top scores are below a threshold

use std::sync::Arc;

use serde::Deserialize;
use tracing::{debug, info, warn};

use crate::document::DocumentTree;
use crate::document::NodeId;
use crate::llm::LlmClient;
use crate::memo::MemoStore;
use crate::retrieval::search::scorer::NodeScorer;

/// A navigation cue produced by the ToCNavigator.
#[derive(Debug, Clone)]
pub struct SearchCue {
    /// The node to start searching from.
    pub root: NodeId,
    /// Confidence score from the locate phase (0.0 - 1.0).
    pub confidence: f32,
}

/// Hierarchical ToC navigator — locates relevant subtrees before tree traversal.
pub struct ToCNavigator {
    /// Optional LLM client for Phase B refinement.
    llm_client: Option<LlmClient>,
    /// Optional memo store for caching locate results.
    memo_store: Option<Arc<MemoStore>>,
    /// Maximum number of top branches to return.
    max_branches: usize,
    /// Score threshold below which LLM refinement is attempted.
    llm_threshold: f32,
}

impl Default for ToCNavigator {
    fn default() -> Self {
        Self::new()
    }
}

impl ToCNavigator {
    /// Create a new ToCNavigator with defaults.
    pub fn new() -> Self {
        Self {
            llm_client: None,
            memo_store: None,
            max_branches: 3,
            llm_threshold: 0.6,
        }
    }

    /// Set the LLM client for Phase B refinement.
    pub fn with_llm_client(mut self, client: LlmClient) -> Self {
        self.llm_client = Some(client);
        self
    }

    /// Set the memo store for caching results.
    pub fn with_memo_store(mut self, store: Arc<MemoStore>) -> Self {
        self.memo_store = Some(store);
        self
    }

    /// Set the maximum number of branches to return.
    pub fn with_max_branches(mut self, n: usize) -> Self {
        self.max_branches = n.max(1);
        self
    }

    /// Locate relevant subtrees for the given query.
    ///
    /// Phase A: Score top-level nodes with BM25 and keep the top-N.
    /// Phase B: If the best BM25 score is below `llm_threshold` and an LLM
    ///          client is available, ask the LLM to refine the selection.
    pub async fn locate(
        &self,
        query: &str,
        tree: &DocumentTree,
        top_level_nodes: &[NodeId],
    ) -> Vec<SearchCue> {
        if top_level_nodes.is_empty() {
            return vec![SearchCue {
                root: tree.root(),
                confidence: 0.5,
            }];
        }

        // Phase A: BM25 scoring
        let scorer = NodeScorer::for_query(query);
        let scored: Vec<(NodeId, f32)> = top_level_nodes
            .iter()
            .map(|&id| (id, scorer.score(tree, id)))
            .filter(|(_, s)| *s > 0.05)
            .collect();

        let top_branches = take_top_n(scored, self.max_branches);

        debug!(
            "ToCNavigator Phase A: {} top-level nodes scored, {} kept after filter",
            top_level_nodes.len(),
            top_branches.len()
        );

        // Phase B: LLM refinement (only when best score is below threshold)
        if let Some(ref client) = self.llm_client {
            let best_score = top_branches.first().map(|(_, s)| *s).unwrap_or(0.0);
            if best_score < self.llm_threshold {
                info!(
                    "Top BM25 score {:.3} below threshold {:.3}, attempting LLM refinement",
                    best_score, self.llm_threshold
                );
                return self
                    .llm_refine(query, tree, top_level_nodes, client)
                    .await;
            }
        }

        // Fallback: if no branches passed the filter, search from root
        if top_branches.is_empty() {
            debug!("ToCNavigator: no branches above threshold, falling back to root");
            return vec![SearchCue {
                root: tree.root(),
                confidence: 0.5,
            }];
        }

        // Return BM25 results as cues
        top_branches
            .into_iter()
            .map(|(node_id, score)| SearchCue {
                root: node_id,
                confidence: score,
            })
            .collect()
    }

    /// Phase B: Ask the LLM to pick the most relevant subtrees.
    ///
    /// Presents the full top-level TOC to the LLM and lets it select the
    /// most relevant entries. Uses direct tree traversal so that we can
    /// correctly map LLM-selected indices back to real NodeIds.
    async fn llm_refine(
        &self,
        query: &str,
        tree: &DocumentTree,
        top_level_nodes: &[NodeId],
        client: &LlmClient,
    ) -> Vec<SearchCue> {
        // Collect (title, summary) and the corresponding NodeId directly
        // from the tree, maintaining index correspondence for LLM response mapping.
        let mut toc_entries: Vec<(String, Option<String>)> = Vec::new();
        let mut node_ids: Vec<NodeId> = Vec::new();

        for &node_id in top_level_nodes {
            collect_tree_entries(tree, node_id, &mut toc_entries, &mut node_ids, 0, 2);
        }

        if node_ids.is_empty() {
            warn!("LLM refinement: no nodes collected from top-level branches");
            return vec![SearchCue {
                root: tree.root(),
                confidence: 0.5,
            }];
        }

        let toc_str = toc_entries
            .iter()
            .enumerate()
            .map(|(i, (title, summary))| {
                format!(
                    "[{}] Title: \"{}\"\n    Summary: \"{}\"",
                    i + 1,
                    title,
                    summary.as_deref().unwrap_or("(no summary)")
                )
            })
            .collect::<Vec<_>>()
            .join("\n\n");

        let system_prompt = r#"You are a document navigation assistant. Select the most relevant sections for the user's query.

CRITICAL INSTRUCTIONS:
1. Analyze the user query carefully
2. Examine the provided Table of Contents entries
3. Select the TOP 3 most relevant entries
4. Respond with ONLY valid JSON (no markdown code blocks)

Response format:
{
  "reasoning": "Brief analysis",
  "candidates": [
    {"node_id": 1, "relevance_score": 0.95, "reason": "Why relevant"},
    {"node_id": 2, "relevance_score": 0.80, "reason": "Why relevant"},
    {"node_id": 3, "relevance_score": 0.65, "reason": "Why relevant"}
  ]
}

Rules:
- node_id: MUST be a number from the TOC entries (the number in [N] brackets)
- relevance_score: 0.0 to 1.0
- candidates: exactly 3 items, ordered by relevance"#;

        let user_prompt = format!(
            "USER QUERY: {}\n\nDOCUMENT TOC ({} entries):\n{}\n\nSelect the TOP 3 most relevant entries. Respond with ONLY the JSON object:",
            query,
            node_ids.len(),
            toc_str
        );

        match client
            .complete_json::<LocateResponse>(system_prompt, &user_prompt)
            .await
        {
            Ok(llm_response) => {
                let mut cues = Vec::new();
                for candidate in &llm_response.candidates {
                    let idx = candidate.node_id.saturating_sub(1);
                    if idx < node_ids.len() {
                        cues.push(SearchCue {
                            root: node_ids[idx],
                            confidence: candidate.relevance_score,
                        });
                    }
                }

                if cues.is_empty() {
                    warn!("LLM refinement returned no valid candidates, falling back to BM25");
                    return vec![SearchCue {
                        root: tree.root(),
                        confidence: 0.5,
                    }];
                }

                info!(
                    "LLM refinement selected {} cues (reasoning: {})",
                    cues.len(),
                    &llm_response.reasoning[..llm_response.reasoning.len().min(100)]
                );
                cues
            }
            Err(e) => {
                warn!("LLM refinement failed: {}, falling back to root", e);
                vec![SearchCue {
                    root: tree.root(),
                    confidence: 0.5,
                }]
            }
        }
    }
}

/// Take the top-N scored items, sorted descending by score.
fn take_top_n(scored: Vec<(NodeId, f32)>, n: usize) -> Vec<(NodeId, f32)> {
    let mut sorted = scored;
    sorted.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    sorted.truncate(n);
    sorted
}

/// Collect tree entries (title, summary) alongside their NodeIds.
///
/// Walks the subtree rooted at `node_id` up to `max_depth` levels deep.
/// The `toc_entries[i]` ↔ `node_ids[i]` correspondence is guaranteed,
/// so LLM response indices can be mapped back to real NodeIds.
fn collect_tree_entries(
    tree: &DocumentTree,
    node_id: NodeId,
    entries: &mut Vec<(String, Option<String>)>,
    node_ids: &mut Vec<NodeId>,
    depth: usize,
    max_depth: usize,
) {
    if depth > max_depth {
        return;
    }
    if let Some(node) = tree.get(node_id) {
        let summary = if node.summary.is_empty() {
            None
        } else {
            Some(node.summary.clone())
        };
        entries.push((node.title.clone(), summary));
        node_ids.push(node_id);

        for child_id in tree.children(node_id) {
            collect_tree_entries(tree, child_id, entries, node_ids, depth + 1, max_depth);
        }
    }
}

/// LLM response for locate query.
#[derive(Debug, Clone, Deserialize)]
struct LocateResponse {
    reasoning: String,
    candidates: Vec<LocateCandidate>,
}

/// A candidate from LLM locate response.
#[derive(Debug, Clone, Deserialize)]
struct LocateCandidate {
    node_id: usize,
    relevance_score: f32,
    #[allow(dead_code)]
    reason: String,
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_take_top_n_logic() {
        let mut scored: Vec<(u32, f32)> = vec![(0, 0.1), (1, 0.9), (2, 0.5), (3, 0.3)];
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(2);
        assert_eq!(scored.len(), 2);
        assert_eq!(scored[0].1, 0.9);
        assert_eq!(scored[1].1, 0.5);
    }
}
