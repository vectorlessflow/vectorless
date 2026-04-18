// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Navigation index for Agent-based retrieval.
//!
//! This is the primary data source for the Agent during the query phase.
//! It provides a compact, pre-computed view of the document tree optimized
//! for navigation decisions — the Agent can decide where to descend without
//! reading the actual content.
//!
//! # Design
//!
//! Based on the Corpus2Skill paper (2604.14572v1), this is the in-memory
//! equivalent of SKILL.md / INDEX.md. The Agent reads `child_routes` at
//! each decision point to see all available sub-topics and their descriptions,
//! then chooses where to navigate next.
//!
//! # Data Flow
//!
//! ```text
//! Enhance stage (writes to TreeNode):
//!   summary, description, routing_keywords, leaf_count
//!       │
//!       └──→ Navigation stage (reads TreeNode fields)
//!             Builds: NavigationIndex (NavEntry + ChildRoute)
//! ```
//!
//! No LLM calls are made during Navigation stage construction.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use super::node::NodeId;

/// Navigation index — Agent's primary data source during the query phase.
///
/// Contains pre-computed navigation metadata for every non-leaf node,
/// allowing the Agent to make routing decisions without accessing the
/// content layer (DocumentTree).
///
/// `HashMap<NodeId, _>` fields use `serde_helpers` (Vec pairs) because
/// serde_json cannot deserialize integer-keyed maps.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NavigationIndex {
    /// Navigation entry for each non-leaf node.
    #[serde(with = "super::serde_helpers")]
    nav_entries: HashMap<NodeId, NavEntry>,

    /// Child routes for each non-leaf node.
    #[serde(with = "super::serde_helpers")]
    child_routes: HashMap<NodeId, Vec<ChildRoute>>,

    /// Pre-computed document card for multi-document Orchestrator.
    /// Built during compile phase by NavigationIndexStage.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    doc_card: Option<DocCard>,
}

impl NavigationIndex {
    /// Create a new empty navigation index.
    pub fn new() -> Self {
        Self {
            nav_entries: HashMap::new(),
            child_routes: HashMap::new(),
            doc_card: None,
        }
    }

    /// Add a navigation entry for a non-leaf node.
    pub fn add_entry(&mut self, node_id: NodeId, entry: NavEntry) {
        self.nav_entries.insert(node_id, entry);
    }

    /// Add child routes for a non-leaf node.
    pub fn add_child_routes(&mut self, parent_id: NodeId, routes: Vec<ChildRoute>) {
        self.child_routes.insert(parent_id, routes);
    }

    /// Get the navigation entry for a node.
    pub fn get_entry(&self, node_id: NodeId) -> Option<&NavEntry> {
        self.nav_entries.get(&node_id)
    }

    /// Get the child routes for a node.
    pub fn get_child_routes(&self, node_id: NodeId) -> Option<&[ChildRoute]> {
        self.child_routes.get(&node_id).map(|v| v.as_slice())
    }

    /// Get the number of navigation entries.
    pub fn entry_count(&self) -> usize {
        self.nav_entries.len()
    }

    /// Get the total number of child route records.
    pub fn total_child_routes(&self) -> usize {
        self.child_routes.values().map(|v| v.len()).sum()
    }

    /// Get the root node's navigation entry.
    pub fn root_entry(&self) -> Option<&NavEntry> {
        // The root should always be present if the index is non-empty.
        // Return the first entry with level 0.
        self.nav_entries
            .values()
            .find(|e| e.level == 0)
    }

    /// Iterate over all navigation entries.
    pub fn entries(&self) -> impl Iterator<Item = (&NodeId, &NavEntry)> {
        self.nav_entries.iter()
    }

    /// Iterate over all child route sets.
    pub fn all_child_routes(&self) -> impl Iterator<Item = (&NodeId, &[ChildRoute])> {
        self.child_routes.iter().map(|(k, v)| (k, v.as_slice()))
    }

    /// Check if the index is empty.
    pub fn is_empty(&self) -> bool {
        self.nav_entries.is_empty()
    }

    /// Get the pre-computed document card.
    pub fn doc_card(&self) -> Option<&DocCard> {
        self.doc_card.as_ref()
    }

    /// Set the document card.
    pub fn set_doc_card(&mut self, card: DocCard) {
        self.doc_card = Some(card);
    }
}

impl Default for NavigationIndex {
    fn default() -> Self {
        Self::new()
    }
}

/// Navigation entry for a non-leaf node.
///
/// Provides the Agent with enough context to decide whether this subtree
/// is relevant to the current query, without needing to read the node's
/// actual content.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NavEntry {
    /// Routing summary describing what this subtree covers.
    /// Comes from Enhance stage's `summary` (routing-oriented).
    pub overview: String,

    /// Typical questions this subtree can answer.
    /// Extracted from content/summary during Enhance stage.
    pub question_hints: Vec<String>,

    /// Topic tags for keyword-based matching.
    /// Comes from Enhance stage's `routing_keywords`.
    pub topic_tags: Vec<String>,

    /// Total number of leaf nodes in this subtree.
    /// Equivalent to the paper's `num_documents`.
    pub leaf_count: usize,

    /// Depth of this node in the tree.
    /// Equivalent to the paper's `level`.
    pub level: usize,
}

/// Child route — compact routing info for one child node.
///
/// The Agent sees a list of `ChildRoute`s when deciding which child
/// to descend into. This provides progressive disclosure: the Agent
/// doesn't need to enter the child node to understand what it contains.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChildRoute {
    /// The child node's ID (for the Agent to navigate to).
    pub node_id: NodeId,

    /// Child node's title.
    pub title: String,

    /// One-sentence description of what this child covers.
    /// Comes from Enhance stage's `description` field.
    pub description: String,

    /// Number of leaf nodes in this child's subtree.
    pub leaf_count: usize,
}

/// Pre-computed document card for multi-document Orchestrator Agent.
///
/// Built during the compile phase by `NavigationIndexStage`, this provides
/// a compact summary of the entire document — enough for the Orchestrator
/// to decide whether a document is relevant to a query without entering it.
///
/// All fields come from data already computed in earlier phases of the
/// NavigationIndexStage (root NavEntry + root child_routes). No LLM calls.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocCard {
    /// Document title (root node title).
    pub title: String,

    /// Document overview (root NavEntry.overview).
    pub overview: String,

    /// Questions this document can answer (root NavEntry.question_hints).
    pub question_hints: Vec<String>,

    /// Topic keywords (root NavEntry.topic_tags).
    pub topic_tags: Vec<String>,

    /// Top-level section summaries (from root child_routes).
    pub sections: Vec<SectionCard>,

    /// Total leaf nodes in the document.
    pub total_leaves: usize,
}

/// One top-level section in a [`DocCard`].
///
/// Provides a compact view of a single top-level section,
/// allowing the Orchestrator to scan section titles and descriptions
/// to assess document relevance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SectionCard {
    /// Section title.
    pub title: String,

    /// One-sentence description of this section.
    pub description: String,

    /// Number of leaf nodes in this section's subtree.
    pub leaf_count: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::DocumentTree;

    fn build_small_tree() -> DocumentTree {
        // Root -> [Child1 (leaf), Child2 -> [Grandchild (leaf)]]
        let mut tree = DocumentTree::new("Root", "");
        let root = tree.root();
        let _child1 = tree.add_child(root, "Child1", "leaf content");
        let child2 = tree.add_child(root, "Child2", "");
        let _grandchild = tree.add_child(child2, "Grandchild", "leaf content");
        tree
    }

    #[test]
    fn test_empty_navigation_index() {
        let index = NavigationIndex::new();
        assert!(index.is_empty());
        assert_eq!(index.entry_count(), 0);
        assert_eq!(index.total_child_routes(), 0);
        assert!(index.root_entry().is_none());
    }

    #[test]
    fn test_add_and_retrieve_entry() {
        let tree = build_small_tree();
        let root = tree.root();

        let entry = NavEntry {
            overview: "Payment integration guide".to_string(),
            question_hints: vec!["How to set up Stripe?".to_string()],
            topic_tags: vec!["payment".to_string(), "stripe".to_string()],
            leaf_count: 5,
            level: 0,
        };

        let mut index = NavigationIndex::new();
        index.add_entry(root, entry);

        assert!(!index.is_empty());
        assert_eq!(index.entry_count(), 1);

        let retrieved = index.get_entry(root).unwrap();
        assert_eq!(retrieved.overview, "Payment integration guide");
        assert_eq!(retrieved.leaf_count, 5);
    }

    #[test]
    fn test_add_and_retrieve_child_routes() {
        let tree = build_small_tree();
        let root = tree.root();
        let children: Vec<NodeId> = tree.children_iter(root).collect();

        let routes = vec![
            ChildRoute {
                node_id: children[0],
                title: "Getting Started".to_string(),
                description: "Setup and installation".to_string(),
                leaf_count: 3,
            },
            ChildRoute {
                node_id: children[1],
                title: "API Reference".to_string(),
                description: "REST API endpoints".to_string(),
                leaf_count: 7,
            },
        ];

        let mut index = NavigationIndex::new();
        index.add_child_routes(root, routes);

        let retrieved = index.get_child_routes(root).unwrap();
        assert_eq!(retrieved.len(), 2);
        assert_eq!(retrieved[0].title, "Getting Started");
        assert_eq!(retrieved[1].leaf_count, 7);
        assert_eq!(index.total_child_routes(), 2);
    }

    #[test]
    fn test_root_entry() {
        let tree = build_small_tree();
        let root = tree.root();
        let children: Vec<NodeId> = tree.children_iter(root).collect();

        let mut index = NavigationIndex::new();
        index.add_entry(
            root,
            NavEntry {
                overview: "Root".to_string(),
                question_hints: vec![],
                topic_tags: vec![],
                leaf_count: 10,
                level: 0,
            },
        );
        index.add_entry(
            children[1],
            NavEntry {
                overview: "Child".to_string(),
                question_hints: vec![],
                topic_tags: vec![],
                leaf_count: 5,
                level: 1,
            },
        );

        let root_entry = index.root_entry().unwrap();
        assert_eq!(root_entry.level, 0);
        assert_eq!(root_entry.leaf_count, 10);
    }

    #[test]
    fn test_get_entry_nonexistent() {
        let index = NavigationIndex::new();
        let tree = build_small_tree();
        // Leaf node should never have an entry
        let children: Vec<NodeId> = tree.children_iter(tree.root()).collect();
        assert!(index.get_entry(children[0]).is_none());
    }

    #[test]
    fn test_get_child_routes_nonexistent() {
        let index = NavigationIndex::new();
        let tree = build_small_tree();
        assert!(index.get_child_routes(tree.root()).is_none());
    }

    #[test]
    fn test_default_trait() {
        let index = NavigationIndex::default();
        assert!(index.is_empty());
    }

    #[test]
    fn test_entries_iterator() {
        let tree = build_small_tree();
        let root = tree.root();
        let children: Vec<NodeId> = tree.children_iter(root).collect();

        let mut index = NavigationIndex::new();
        index.add_entry(
            root,
            NavEntry {
                overview: "Root".to_string(),
                question_hints: vec![],
                topic_tags: vec![],
                leaf_count: 2,
                level: 0,
            },
        );
        index.add_entry(
            children[1], // Child2 is non-leaf
            NavEntry {
                overview: "Child2".to_string(),
                question_hints: vec![],
                topic_tags: vec![],
                leaf_count: 1,
                level: 1,
            },
        );

        let all_entries: Vec<_> = index.entries().collect();
        assert_eq!(all_entries.len(), 2);
    }

    #[test]
    fn test_all_child_routes_iterator() {
        let tree = build_small_tree();
        let root = tree.root();
        let children: Vec<NodeId> = tree.children_iter(root).collect();

        let mut index = NavigationIndex::new();
        index.add_child_routes(
            root,
            vec![ChildRoute {
                node_id: children[0],
                title: "C1".to_string(),
                description: "d".to_string(),
                leaf_count: 1,
            }],
        );

        let all_routes: Vec<_> = index.all_child_routes().collect();
        assert_eq!(all_routes.len(), 1);
        assert_eq!(all_routes[0].1.len(), 1);
    }

    #[test]
    fn test_serialization_roundtrip() {
        let tree = build_small_tree();
        let root = tree.root();
        let children: Vec<NodeId> = tree.children_iter(root).collect();

        let mut index = NavigationIndex::new();
        index.add_entry(
            root,
            NavEntry {
                overview: "Root overview".to_string(),
                question_hints: vec!["What is this?".to_string()],
                topic_tags: vec!["intro".to_string(), "guide".to_string()],
                leaf_count: 2,
                level: 0,
            },
        );
        index.add_child_routes(
            root,
            vec![
                ChildRoute {
                    node_id: children[0],
                    title: "Child1".to_string(),
                    description: "First child desc".to_string(),
                    leaf_count: 1,
                },
                ChildRoute {
                    node_id: children[1],
                    title: "Child2".to_string(),
                    description: "Second child desc".to_string(),
                    leaf_count: 1,
                },
            ],
        );

        // Serialize
        let json = serde_json::to_string(&index).expect("serialization failed");

        // Deserialize
        let deserialized: NavigationIndex =
            serde_json::from_str(&json).expect("deserialization failed");

        // Verify data survived round-trip
        assert_eq!(deserialized.entry_count(), 1);
        assert_eq!(deserialized.total_child_routes(), 2);

        let entry = deserialized.get_entry(root).unwrap();
        assert_eq!(entry.overview, "Root overview");
        assert_eq!(entry.question_hints.len(), 1);
        assert_eq!(entry.topic_tags.len(), 2);
        assert_eq!(entry.leaf_count, 2);
        assert_eq!(entry.level, 0);

        let routes = deserialized.get_child_routes(root).unwrap();
        assert_eq!(routes[0].title, "Child1");
        assert_eq!(routes[1].title, "Child2");
    }

    #[test]
    fn test_doc_card_default_none() {
        let index = NavigationIndex::new();
        assert!(index.doc_card().is_none());
    }

    #[test]
    fn test_doc_card_set_and_get() {
        let card = DocCard {
            title: "Test Doc".to_string(),
            overview: "A test document".to_string(),
            question_hints: vec!["What?".to_string()],
            topic_tags: vec!["test".to_string()],
            sections: vec![SectionCard {
                title: "Section 1".to_string(),
                description: "First section".to_string(),
                leaf_count: 5,
            }],
            total_leaves: 5,
        };

        let mut index = NavigationIndex::new();
        index.set_doc_card(card);

        let retrieved = index.doc_card().unwrap();
        assert_eq!(retrieved.title, "Test Doc");
        assert_eq!(retrieved.overview, "A test document");
        assert_eq!(retrieved.question_hints.len(), 1);
        assert_eq!(retrieved.topic_tags.len(), 1);
        assert_eq!(retrieved.sections.len(), 1);
        assert_eq!(retrieved.sections[0].title, "Section 1");
        assert_eq!(retrieved.sections[0].leaf_count, 5);
        assert_eq!(retrieved.total_leaves, 5);
    }

    #[test]
    fn test_doc_card_serialization_roundtrip() {
        let tree = build_small_tree();
        let root = tree.root();
        let children: Vec<NodeId> = tree.children_iter(root).collect();

        let mut index = NavigationIndex::new();
        index.add_entry(
            root,
            NavEntry {
                overview: "Root overview".to_string(),
                question_hints: vec!["What is this?".to_string()],
                topic_tags: vec!["intro".to_string()],
                leaf_count: 2,
                level: 0,
            },
        );
        index.add_child_routes(
            root,
            vec![
                ChildRoute {
                    node_id: children[0],
                    title: "Child1".to_string(),
                    description: "First".to_string(),
                    leaf_count: 1,
                },
                ChildRoute {
                    node_id: children[1],
                    title: "Child2".to_string(),
                    description: "Second".to_string(),
                    leaf_count: 1,
                },
            ],
        );

        // Build DocCard from index data
        let root_entry = index.get_entry(root).unwrap();
        let sections: Vec<SectionCard> = index
            .get_child_routes(root)
            .unwrap()
            .iter()
            .map(|r| SectionCard {
                title: r.title.clone(),
                description: r.description.clone(),
                leaf_count: r.leaf_count,
            })
            .collect();
        index.set_doc_card(DocCard {
            title: "Root".to_string(),
            overview: root_entry.overview.clone(),
            question_hints: root_entry.question_hints.clone(),
            topic_tags: root_entry.topic_tags.clone(),
            sections,
            total_leaves: root_entry.leaf_count,
        });

        // Serialize + deserialize
        let json = serde_json::to_string(&index).expect("serialization failed");
        let deserialized: NavigationIndex =
            serde_json::from_str(&json).expect("deserialization failed");

        // Verify DocCard survived round-trip
        let card = deserialized.doc_card().unwrap();
        assert_eq!(card.title, "Root");
        assert_eq!(card.overview, "Root overview");
        assert_eq!(card.question_hints, vec!["What is this?"]);
        assert_eq!(card.topic_tags, vec!["intro"]);
        assert_eq!(card.sections.len(), 2);
        assert_eq!(card.sections[0].title, "Child1");
        assert_eq!(card.sections[1].leaf_count, 1);
        assert_eq!(card.total_leaves, 2);
    }

    #[test]
    fn test_doc_card_backward_compat_deserialize_without_card() {
        // JSON from an older version that doesn't have doc_card
        let tree = build_small_tree();
        let root = tree.root();

        let mut index = NavigationIndex::new();
        index.add_entry(
            root,
            NavEntry {
                overview: "Old index".to_string(),
                question_hints: vec![],
                topic_tags: vec![],
                leaf_count: 2,
                level: 0,
            },
        );
        // No doc_card set

        let json = serde_json::to_string(&index).expect("serialization failed");
        let deserialized: NavigationIndex =
            serde_json::from_str(&json).expect("deserialization failed");

        assert!(deserialized.doc_card().is_none());
        assert_eq!(deserialized.entry_count(), 1);
    }

    #[test]
    fn test_section_card_fields() {
        let card = SectionCard {
            title: "Getting Started".to_string(),
            description: "Quick setup guide".to_string(),
            leaf_count: 3,
        };
        assert_eq!(card.title, "Getting Started");
        assert_eq!(card.description, "Quick setup guide");
        assert_eq!(card.leaf_count, 3);
    }
}
