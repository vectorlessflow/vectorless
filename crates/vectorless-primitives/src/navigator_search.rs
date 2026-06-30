// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0
//
// Search tools — content search and reasoning index queries.
// Included into navigator.rs via `include!`.

impl DocumentNavigator {
    // -----------------------------------------------------------------------
    // Content search
    // -----------------------------------------------------------------------

    /// Regex search across all node content in the current subtree.
    /// Returns up to 30 matches.
    pub async fn grep(&self, pattern: &str) -> Result<Vec<MatchResult>> {
        let re = regex::Regex::new(pattern)
            .map_err(|e| Error::InvalidInput(format!("Invalid regex '{pattern}': {e}")))?;

        let subtree = collect_subtree(self.cursor, &self.doc.tree);
        let mut results = Vec::new();
        let max_matches = 30;

        for node_id in &subtree {
            if results.len() >= max_matches {
                break;
            }
            let content = match self.doc.tree.get(*node_id).map(|n| n.content.as_str()) {
                Some(c) if !c.is_empty() => c,
                _ => continue,
            };
            let title = self
                .doc
                .tree
                .get(*node_id)
                .map(|n| n.title.as_str())
                .unwrap_or("?");

            for (i, line) in content.lines().enumerate() {
                if results.len() >= max_matches {
                    break;
                }
                if re.is_match(line) {
                    results.push(MatchResult {
                        node_id: self.id_to_u64(*node_id),
                        title: title.to_string(),
                        snippet: line.to_string(),
                        line_number: i + 1,
                    });
                }
            }
        }

        Ok(results)
    }

    /// Search for nodes by keyword in title or content (case-insensitive).
    pub async fn find(&self, keyword: &str) -> Vec<FindResult> {
        let kw = keyword.to_lowercase();
        self.doc
            .tree
            .traverse()
            .iter()
            .filter_map(|&id| {
                let node = self.doc.tree.get(id)?;
                if node.title.to_lowercase().contains(&kw)
                    || node.content.to_lowercase().contains(&kw)
                {
                    let depth = self.doc.tree.depth(id);
                    let leaf_count = self
                        .doc
                        .nav_index
                        .get_entry(id)
                        .map(|e| e.leaf_count)
                        .unwrap_or(0);
                    Some(FindResult {
                        node_id: self.id_to_u64(id),
                        title: node.title.clone(),
                        depth,
                        leaf_count,
                    })
                } else {
                    None
                }
            })
            .collect()
    }

    /// Ranked full-text search across the whole document (BM25).
    ///
    /// Returns the top `limit` nodes by relevance to the multi-term query, each
    /// with a snippet. This is the lexical-recall counterpart to keyword/route
    /// lookups: the agent understands the question, then `search` ranks where to
    /// read. Pure compute — no LLM, no precomputed index required.
    pub async fn search(&self, query: &str, limit: usize) -> Vec<SearchHit> {
        // De-duplicated query terms, original order preserved.
        let mut q_terms: Vec<String> = Vec::new();
        for t in Self::bm25_tokenize(query) {
            if !q_terms.contains(&t) {
                q_terms.push(t);
            }
        }
        if q_terms.is_empty() {
            return Vec::new();
        }
        let limit = if limit == 0 { 10 } else { limit };

        // Per-node: length + term frequencies (query terms only).
        let mut nodes: Vec<(NodeId, usize, Vec<u32>)> = Vec::new();
        let mut df = vec![0usize; q_terms.len()];
        let mut total_len = 0usize;

        for id in self.doc.tree.traverse() {
            let node = match self.doc.tree.get(id) {
                Some(n) => n,
                None => continue,
            };
            let mut len = 0usize;
            let mut tf = vec![0u32; q_terms.len()];
            for field in [
                node.title.as_str(),
                node.summary.as_str(),
                node.content.as_str(),
            ] {
                for tok in Self::bm25_tokenize(field) {
                    len += 1;
                    if let Some(i) = q_terms.iter().position(|t| t == &tok) {
                        tf[i] += 1;
                    }
                }
            }
            if len == 0 {
                continue;
            }
            for (i, &c) in tf.iter().enumerate() {
                if c > 0 {
                    df[i] += 1;
                }
            }
            total_len += len;
            nodes.push((id, len, tf));
        }

        let n = nodes.len();
        if n == 0 {
            return Vec::new();
        }
        let avgdl = total_len as f64 / n as f64;
        let k1 = 1.5_f64;
        let b = 0.75_f64;
        let idf: Vec<f64> = df
            .iter()
            .map(|&d| ((n as f64 - d as f64 + 0.5) / (d as f64 + 0.5) + 1.0).ln())
            .collect();

        let mut scored: Vec<(NodeId, f64)> = Vec::new();
        for (id, len, tf) in &nodes {
            let mut score = 0.0_f64;
            for i in 0..q_terms.len() {
                let f = tf[i] as f64;
                if f == 0.0 {
                    continue;
                }
                let denom = f + k1 * (1.0 - b + b * (*len as f64 / avgdl));
                score += idf[i] * (f * (k1 + 1.0)) / denom;
            }
            if score > 0.0 {
                scored.push((*id, score));
            }
        }

        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(limit);

        scored
            .into_iter()
            .filter_map(|(id, score)| {
                let node = self.doc.tree.get(id)?;
                Some(SearchHit {
                    node_id: self.id_to_u64(id),
                    title: node.title.clone(),
                    score,
                    snippet: Self::bm25_snippet(&node.content, &q_terms),
                })
            })
            .collect()
    }

    fn bm25_tokenize(text: &str) -> Vec<String> {
        let mut out = Vec::new();
        let mut cur = String::new();
        for ch in text.chars() {
            if ch.is_alphanumeric() {
                for c in ch.to_lowercase() {
                    cur.push(c);
                }
            } else if !cur.is_empty() {
                if cur.len() >= 2 && !Self::bm25_is_stopword(&cur) {
                    out.push(std::mem::take(&mut cur));
                } else {
                    cur.clear();
                }
            }
        }
        if cur.len() >= 2 && !Self::bm25_is_stopword(&cur) {
            out.push(cur);
        }
        out
    }

    fn bm25_is_stopword(w: &str) -> bool {
        matches!(
            w,
            "the" | "and" | "for" | "are" | "with" | "that" | "this" | "from"
                | "what" | "how" | "was" | "were" | "has" | "have" | "had"
                | "you" | "your" | "all" | "any" | "can" | "will" | "its"
                | "into" | "than" | "then" | "there" | "their" | "which"
        )
    }

    fn bm25_snippet(content: &str, terms: &[String]) -> String {
        let trimmed = content.trim();
        if trimmed.is_empty() {
            return String::new();
        }
        let lower = trimmed.to_lowercase();
        let mut byte_pos: Option<usize> = None;
        for t in terms {
            if let Some(p) = lower.find(t.as_str()) {
                byte_pos = Some(byte_pos.map_or(p, |x| x.min(p)));
            }
        }
        let char_start = match byte_pos {
            Some(p) => lower[..p].chars().count(),
            None => 0,
        };
        let chars: Vec<char> = trimmed.chars().collect();
        let start = char_start.saturating_sub(50);
        let end = (start + 200).min(chars.len());
        let mut snip: String = chars[start..end].iter().collect();
        snip = snip.split_whitespace().collect::<Vec<_>>().join(" ");
        if start > 0 {
            snip = format!("…{snip}");
        }
        if end < chars.len() {
            snip.push('…');
        }
        snip
    }

    /// Search within a specific node's content without moving the cursor.
    pub async fn grep_node(
        &self,
        node_id: &str,
        pattern: &str,
    ) -> Result<Vec<MatchResult>> {
        let id = self.parse_id(node_id)?;
        let re = regex::Regex::new(pattern)
            .map_err(|e| Error::InvalidInput(format!("Invalid regex '{pattern}': {e}")))?;

        let node = self
            .doc
            .tree
            .get(id)
            .ok_or_else(|| Error::NodeNotFound("Node not found.".into()))?;

        let title = node.title.clone();
        let content = &node.content;
        let mut results = Vec::new();

        for (i, line) in content.lines().enumerate() {
            if results.len() >= 30 {
                break;
            }
            if re.is_match(line) {
                results.push(MatchResult {
                    node_id: self.id_to_u64(id),
                    title: title.clone(),
                    snippet: line.to_string(),
                    line_number: i + 1,
                });
            }
        }

        Ok(results)
    }

    /// Find semantically similar nodes using the reasoning index.
    pub async fn similar(&self, node_id: &str) -> Vec<SimilarResult> {
        let id = match self.parse_id(node_id) {
            Ok(id) => id,
            Err(_) => return Vec::new(),
        };

        // Reverse lookup: find all keywords that point to the reference node
        let ref_id_u64 = self.id_to_u64(id);
        let mut ref_keywords: Vec<String> = Vec::new();
        for (kw, entries) in self.doc.reasoning_index.all_topic_entries() {
            if entries.iter().any(|e| self.id_to_u64(e.node_id) == ref_id_u64) {
                ref_keywords.push(kw.clone());
            }
        }

        if ref_keywords.is_empty() {
            return Vec::new();
        }

        // Find all nodes that share keywords with the reference
        let mut candidates: HashMap<u64, (f32, Vec<String>)> = HashMap::new();
        for kw in &ref_keywords {
            if let Some(entries) = self.doc.reasoning_index.topic_entries(kw) {
                for entry in entries {
                    let cid = self.id_to_u64(entry.node_id);
                    if cid == ref_id_u64 {
                        continue;
                    }
                    let (weight, keywords) = candidates.entry(cid).or_insert((0.0, Vec::new()));
                    *weight += entry.weight;
                    keywords.push(kw.clone());
                }
            }
        }

        let mut results: Vec<SimilarResult> = candidates
            .into_iter()
            .filter_map(|(cid, (weight, shared))| {
                let nav_id = self.node_id_map.get(&cid)?;
                let title = self.doc.tree.get(*nav_id).map(|n| n.title.clone())?;
                Some(SimilarResult {
                    id: cid,
                    title,
                    relevance: weight,
                    shared_keywords: shared,
                })
            })
            .collect();

        results.sort_by(|a, b| {
            b.relevance
                .partial_cmp(&a.relevance)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        results.truncate(10);
        results
    }

    /// Get the pre-computed overview for a section from the navigation index.
    pub async fn section_overview(&self, node_id: &str) -> Result<String> {
        let id = self.parse_id(node_id)?;
        let entry = self
            .doc
            .nav_index
            .get_entry(id)
            .ok_or_else(|| Error::NodeNotFound("No nav entry for this node.".into()))?;
        Ok(entry.overview.clone())
    }

    // -----------------------------------------------------------------------
    // Reasoning index queries
    // -----------------------------------------------------------------------

    /// Look up topic entries for a keyword in the reasoning index.
    pub async fn keyword_entries(&self, keyword: &str) -> Vec<TopicEntryInfo> {
        self.doc
            .reasoning_index
            .topic_entries(keyword)
            .map(|entries| {
                entries
                    .iter()
                    .map(|e| TopicEntryInfo {
                        node_id: self.id_to_u64(e.node_id),
                        weight: e.weight,
                        depth: e.depth,
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Section summaries from the reasoning index.
    pub async fn topic_summary(&self) -> Vec<SectionSummaryInfo> {
        self.doc
            .reasoning_index
            .summary_shortcut()
            .map(|sc| {
                sc.section_summaries
                    .iter()
                    .map(|s| SectionSummaryInfo {
                        node_id: self.id_to_u64(s.node_id),
                        title: s.title.clone(),
                        summary: s.summary.clone(),
                        depth: s.depth,
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Find sections related to any of the given keywords.
    pub async fn related_sections(&self, keywords: &[String]) -> Vec<u64> {
        let mut seen = HashSet::new();
        let mut result = Vec::new();
        for kw in keywords {
            if let Some(entries) = self.doc.reasoning_index.topic_entries(kw) {
                for entry in entries {
                    let id = self.id_to_u64(entry.node_id);
                    if seen.insert(id) {
                        result.push(id);
                    }
                }
            }
        }
        result
    }

    // -----------------------------------------------------------------------
    // Agent acceleration queries
    // -----------------------------------------------------------------------

    /// Get all intent routes from the query routing table.
    pub async fn intent_routes(&self) -> Vec<RouteTargetInfo> {
        self.doc
            .query_routes
            .as_ref()
            .map(|table| {
                table
                    .intent_routes()
                    .values()
                    .flat_map(|targets| {
                        targets.iter().map(|t| RouteTargetInfo {
                            node_id: self.id_to_u64(t.node_id),
                            relevance: t.relevance,
                            reason: t.reason.clone(),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Get concept routes matching a keyword.
    pub async fn concept_routes(&self, keyword: &str) -> Vec<ConceptRouteInfo> {
        let targets: Vec<RouteTargetInfo> = self
            .doc
            .query_routes
            .as_ref()
            .map(|table| {
                table
                    .routes_for_concept(keyword)
                    .into_iter()
                    .map(|t| RouteTargetInfo {
                        node_id: self.id_to_u64(t.node_id),
                        relevance: t.relevance,
                        reason: t.reason.clone(),
                    })
                    .collect()
            })
            .unwrap_or_default();

        if targets.is_empty() {
            return Vec::new();
        }

        vec![ConceptRouteInfo {
            concept: keyword.to_string(),
            targets,
        }]
    }

    /// Get reasoning chains involving a specific node.
    pub async fn chains_for(&self, node_id: u64) -> Vec<ChainInfo> {
        let nid = match self.u64_to_id(node_id) {
            Some(id) => id,
            None => return Vec::new(),
        };
        self.doc
            .chain_index
            .as_ref()
            .map(|idx| {
                idx.chains_for(nid)
                    .into_iter()
                    .map(|c| ChainInfo {
                        premises: c.premises.iter().map(|&id| self.id_to_u64(id)).collect(),
                        conclusions: c.conclusions.iter().map(|&id| self.id_to_u64(id)).collect(),
                        chain_type: c.chain_type.as_str().to_string(),
                        summary: c.summary.clone(),
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Get overlapping nodes for a specific node.
    pub async fn overlaps_for(&self, node_id: u64) -> Vec<OverlapInfo> {
        let nid = match self.u64_to_id(node_id) {
            Some(id) => id,
            None => return Vec::new(),
        };
        self.doc
            .content_overlap
            .as_ref()
            .map(|map| {
                map.overlapping_nodes(nid)
                    .into_iter()
                    .map(|(id, sim, ot)| OverlapInfo {
                        node_a: node_id,
                        node_b: self.id_to_u64(id),
                        similarity: sim,
                        overlap_type: match ot {
                            vectorless_document::OverlapType::Duplicate => "duplicate",
                            vectorless_document::OverlapType::Subset => "subset",
                            vectorless_document::OverlapType::Summary => "summary",
                        }
                        .to_string(),
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Get evidence quality score for a specific node.
    /// Compile-time routing signal for a node: summary, routing keywords, and the
    /// questions this subtree can answer (populated by the enrich stage). Lets an
    /// agent's planner judge a section without reading its full content.
    pub async fn node_routing(&self, node_id: u64) -> Option<NodeRoutingInfo> {
        let nid = self.u64_to_id(node_id)?;
        let node = self.doc.tree.get(nid)?;
        Some(NodeRoutingInfo {
            node_id,
            title: node.title.clone(),
            summary: node.summary.clone(),
            keywords: node.routing_keywords.clone(),
            questions: node.question_hints.clone(),
        })
    }

    pub async fn evidence_score_for(&self, node_id: u64) -> Option<EvidenceScoreInfo> {
        let nid = self.u64_to_id(node_id)?;
        self.doc.evidence_scores.as_ref()?.get(nid).map(|s| EvidenceScoreInfo {
            node_id,
            density: s.density,
            data_richness: s.data_richness,
            specificity: s.specificity,
            composite: s.composite(),
        })
    }

    /// Get all evidence scores, sorted by composite descending.
    pub async fn evidence_scores_ranked(&self) -> Vec<EvidenceScoreInfo> {
        self.doc
            .evidence_scores
            .as_ref()
            .map(|map| {
                map.ranked_nodes()
                    .into_iter()
                    .filter_map(|(nid, composite)| {
                        map.get(nid).map(|s| EvidenceScoreInfo {
                            node_id: self.id_to_u64(nid),
                            density: s.density,
                            data_richness: s.data_richness,
                            specificity: s.specificity,
                            composite,
                        })
                    })
                    .collect()
            })
            .unwrap_or_default()
    }
}
