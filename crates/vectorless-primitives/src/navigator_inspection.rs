// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0
//
// P1 inspection tools — structural and metadata queries.
// Included into navigator.rs via `include!`.

impl DocumentNavigator {
    // -----------------------------------------------------------------------
    // P1: Inspection tools
    // -----------------------------------------------------------------------

    /// Return the full table of contents as a flat list of entries.
    pub async fn toc(&self) -> Vec<TocEntry> {
        fn walk(
            tree: &vectorless_document::DocumentTree,
            node_id: NodeId,
            depth: usize,
            entries: &mut Vec<TocEntry>,
        ) {
            if depth > 0 {
                // skip root
                let child_count = tree.children(node_id).len();
                let title = tree.get(node_id).map(|n| n.title.clone()).unwrap_or_default();
                let id_u64 = usize::from(node_id.0) as u64;
                entries.push(TocEntry {
                    id: id_u64,
                    title,
                    depth,
                    child_count,
                });
            }
            for child in tree.children(node_id) {
                walk(tree, child, depth + 1, entries);
            }
        }
        let mut entries = Vec::new();
        walk(&self.doc.tree, self.doc.tree.root(), 0, &mut entries);
        entries
    }

    /// Get statistics about a node (or the current node if None).
    pub async fn stats(&self, node_id: Option<&str>) -> Result<NodeStats> {
        let id = self.resolve_optional_id(node_id)?;
        let node = self
            .doc
            .tree
            .get(id)
            .ok_or_else(|| Error::NodeNotFound("Node not found.".into()))?;

        let children = self.doc.tree.children(id);
        let depth = self.doc.tree.depth(id);
        let leaf_count = self
            .doc
            .nav_index
            .get_entry(id)
            .map(|e| e.leaf_count)
            .unwrap_or(0);

        Ok(NodeStats {
            id: self.id_to_u64(id),
            title: node.title.clone(),
            depth,
            child_count: children.len(),
            leaf_count,
            char_count: node.content.len(),
            word_count: node.content.split_whitespace().count(),
            is_leaf: children.is_empty(),
        })
    }

    /// List sibling nodes at the same level as a given node (or current node).
    pub async fn siblings(&self, node_id: Option<&str>) -> Result<Vec<NodeInfo>> {
        let id = self.resolve_optional_id(node_id)?;
        let mut result = Vec::new();
        for sibling_id in self.doc.tree.siblings_iter(id) {
            let child_count = self.doc.tree.children(sibling_id).len();
            let (hints, tags, leaf_count) = self
                .doc
                .nav_index
                .get_entry(sibling_id)
                .map(|e| (e.question_hints.clone(), e.topic_tags.clone(), e.leaf_count))
                .unwrap_or_default();
            let depth = self.doc.tree.depth(sibling_id);
            let title = self
                .doc
                .tree
                .get(sibling_id)
                .map(|n| n.title.clone())
                .unwrap_or_default();
            result.push(NodeInfo {
                id: self.id_to_u64(sibling_id),
                title,
                depth,
                child_count,
                leaf_count,
                question_hints: hints,
                topic_tags: tags,
            });
        }
        Ok(result)
    }

    /// List ancestors from root to the current (or specified) node, inclusive.
    pub async fn ancestors(&self, node_id: Option<&str>) -> Result<Vec<NodeInfo>> {
        let id = self.resolve_optional_id(node_id)?;
        let path = self.doc.tree.path_from_root(id);
        let mut result = Vec::new();
        for path_id in &path {
            let child_count = self.doc.tree.children(*path_id).len();
            let (hints, tags, leaf_count) = self
                .doc
                .nav_index
                .get_entry(*path_id)
                .map(|e| (e.question_hints.clone(), e.topic_tags.clone(), e.leaf_count))
                .unwrap_or_default();
            let depth = self.doc.tree.depth(*path_id);
            let title = self
                .doc
                .tree
                .get(*path_id)
                .map(|n| n.title.clone())
                .unwrap_or_default();
            result.push(NodeInfo {
                id: self.id_to_u64(*path_id),
                title,
                depth,
                child_count,
                leaf_count,
                question_hints: hints,
                topic_tags: tags,
            });
        }
        Ok(result)
    }

    /// Document-level overview card (title, overview, sections, concepts).
    pub async fn doc_card(&self) -> Option<DocCardInfo> {
        self.doc.nav_index.doc_card().map(|card| DocCardInfo {
            title: card.title.clone(),
            overview: card.overview.clone(),
            question_hints: card.question_hints.clone(),
            topic_tags: card.topic_tags.clone(),
            sections: card
                .sections
                .iter()
                .map(|s| SectionCardInfo {
                    title: s.title.clone(),
                    description: s.description.clone(),
                    leaf_count: s.leaf_count,
                })
                .collect(),
            total_leaves: card.total_leaves,
        })
    }

    /// Key concepts extracted from the document.
    pub async fn concepts(&self) -> Vec<ConceptInfo> {
        self.doc
            .concepts
            .iter()
            .map(|c| ConceptInfo {
                name: c.name.clone(),
                summary: c.summary.clone(),
                sections: c.sections.clone(),
            })
            .collect()
    }

    /// Find a section by exact title (case-insensitive).
    pub async fn find_section(&self, title: &str) -> Option<FindResult> {
        let id = self.doc.reasoning_index.find_section(title)?;
        let node = self.doc.tree.get(id)?;
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
    }
}
