// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Read-only data access wrappers over compile artifacts.
//!
//! These types provide the agent with structured access to the document's
//! navigation index, content tree, and reasoning index — all read-only.

use crate::document::{ChildRoute, NodeId, TopicEntry};

// Re-export from config for convenience
pub use super::config::{DocContext, WorkspaceContext};

/// A single hit from a keyword search.
#[derive(Debug, Clone)]
pub struct FindHit {
    /// The matched keyword.
    pub keyword: String,
    /// Topic entries matching the keyword.
    pub entries: Vec<TopicEntry>,
}

impl<'a> DocContext<'a> {
    /// List child routes for a given node.
    pub fn ls(&self, node: NodeId) -> Option<&[ChildRoute]> {
        self.nav_index.get_child_routes(node)
    }

    /// Read the full content of a node.
    pub fn cat(&self, node: NodeId) -> Option<&str> {
        self.tree.get(node).map(|n| n.content.as_str())
    }

    /// Get the title of a node.
    pub fn node_title(&self, node: NodeId) -> Option<&str> {
        self.tree.get(node).map(|n| n.title.as_str())
    }

    /// Search for a keyword in the reasoning index.
    pub fn find(&self, keyword: &str) -> Option<FindHit> {
        self.reasoning_index
            .topic_entries(keyword)
            .map(|entries| FindHit {
                keyword: keyword.to_string(),
                entries: entries.to_vec(),
            })
    }

    /// Search for multiple keywords, collecting all hits.
    pub fn find_all(&self, keywords: &[String]) -> Vec<FindHit> {
        keywords
            .iter()
            .filter_map(|kw| self.find(kw))
            .collect()
    }

    /// Get the root node ID.
    pub fn root(&self) -> NodeId {
        self.tree.root()
    }

    /// Get the document's DocCard, if available.
    pub fn doc_card(&self) -> Option<&crate::document::DocCard> {
        self.nav_index.doc_card()
    }

    /// Get the navigation entry for a node (overview, hints, tags).
    pub fn nav_entry(&self, node: NodeId) -> Option<&crate::document::NavEntry> {
        self.nav_index.get_entry(node)
    }

    /// Get the parent of a node (by searching the tree).
    pub fn parent(&self, node: NodeId) -> Option<NodeId> {
        self.tree.parent(node)
    }
}

impl<'a> WorkspaceContext<'a> {
    /// Search for a keyword across all documents.
    pub fn find_cross(&self, keyword: &str) -> Vec<(usize, FindHit)> {
        self.docs
            .iter()
            .enumerate()
            .filter_map(|(idx, doc)| {
                doc.find(keyword).map(|hit| (idx, hit))
            })
            .collect()
    }

    /// Search multiple keywords across all documents.
    pub fn find_cross_all(&self, keywords: &[String]) -> Vec<(usize, Vec<FindHit>)> {
        let mut results: Vec<(usize, Vec<FindHit>)> = Vec::new();
        for (idx, doc) in self.docs.iter().enumerate() {
            let hits = doc.find_all(keywords);
            if !hits.is_empty() {
                results.push((idx, hits));
            }
        }
        results
    }

    /// Get all DocCards for documents that have them.
    pub fn doc_cards(&self) -> Vec<(usize, &crate::document::DocCard)> {
        self.docs
            .iter()
            .enumerate()
            .filter_map(|(idx, doc)| doc.doc_card().map(|card| (idx, card)))
            .collect()
    }

    /// Get a specific document context by index.
    pub fn doc(&self, idx: usize) -> Option<&DocContext<'a>> {
        self.docs.get(idx)
    }
}
