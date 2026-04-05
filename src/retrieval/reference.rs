// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Reference following for in-document cross-references.
//!
//! This module implements the ability to follow references found within
//! document content, such as "see Appendix G" or "refer to Table 5.3".
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │                   ReferenceFollower                          │
//! ├─────────────────────────────────────────────────────────────┤
//! │                                                              │
//! │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐         │
//! │  │ Extract     │─▶│ Resolve     │─▶│ Expand      │         │
//! │  │ References  │  │ References  │  │ Context     │         │
//! │  └─────────────┘  └─────────────┘  └─────────────┘         │
//! │                                                              │
//! │  Features:                                                   │
//! │  • Follow "see Section X" references                        │
//! │  • Follow "see Appendix G" references                       │
//! │  • Follow "Table/Figure X" references                       │
//! │  • Depth-limited expansion                                  │
//! │  • Reference cycle detection                                │
//! └─────────────────────────────────────────────────────────────┘
//! ```
//!
//! # Integration with Retrieval
//!
//! Reference following is triggered when:
//! 1. Search finds content containing references
//! 2. Judge determines current content is insufficient
//! 3. Pilot suggests following a specific reference
//!
//! # Example
//!
//! ```ignore
//! use vectorless::retrieval::reference::{ReferenceFollower, ReferenceConfig};
//!
//! let follower = ReferenceFollower::new(ReferenceConfig {
//!     max_depth: 3,
//!     max_references: 10,
//!     ..Default::default()
//! });
//!
//! // Follow references from a node
//! let expanded = follower.follow_from_node(&tree, &index, node_id, &query);
//! for (ref_node_id, ref_text) in expanded {
//!     println!("Found referenced node: {} via '{}'", ref_node_id, ref_text);
//! }
//! ```

use std::collections::{HashMap, HashSet};

use crate::document::{
    DocumentTree, NodeId, NodeReference, RefType, ReferenceExtractor, RetrievalIndex,
};

/// Configuration for reference following.
#[derive(Debug, Clone)]
pub struct ReferenceConfig {
    /// Maximum depth for following chained references.
    pub max_depth: usize,
    /// Maximum total references to follow per query.
    pub max_references: usize,
    /// Whether to follow page references.
    pub follow_pages: bool,
    /// Whether to follow table/figure references.
    pub follow_tables_figures: bool,
    /// Minimum confidence threshold for resolution.
    pub min_confidence: f32,
    /// Reference types to include.
    pub include_types: Vec<RefType>,
}

impl Default for ReferenceConfig {
    fn default() -> Self {
        Self {
            max_depth: 3,
            max_references: 10,
            follow_pages: true,
            follow_tables_figures: true,
            min_confidence: 0.5,
            include_types: vec![
                RefType::Section,
                RefType::Appendix,
                RefType::Table,
                RefType::Figure,
                RefType::Page,
            ],
        }
    }
}

impl ReferenceConfig {
    /// Create a conservative configuration (fewer references).
    pub fn conservative() -> Self {
        Self {
            max_depth: 2,
            max_references: 5,
            ..Default::default()
        }
    }

    /// Create an aggressive configuration (more references).
    pub fn aggressive() -> Self {
        Self {
            max_depth: 5,
            max_references: 20,
            ..Default::default()
        }
    }

    /// Check if a reference type should be followed.
    pub fn should_follow(&self, ref_type: RefType) -> bool {
        if !self.include_types.contains(&ref_type) {
            return false;
        }
        match ref_type {
            RefType::Page => self.follow_pages,
            RefType::Table | RefType::Figure => self.follow_tables_figures,
            _ => true,
        }
    }
}

/// Result of following a reference.
#[derive(Debug, Clone)]
pub struct FollowedReference {
    /// The node that contained the reference.
    pub source_node: NodeId,
    /// The reference that was followed.
    pub reference: NodeReference,
    /// The resolved target node (if found).
    pub target_node: Option<NodeId>,
    /// Depth in the reference chain (0 = direct from content).
    pub depth: usize,
}

impl FollowedReference {
    /// Check if this reference was resolved.
    pub fn is_resolved(&self) -> bool {
        self.target_node.is_some()
    }
}

/// Reference follower for expanding content via cross-references.
///
/// This implements the PageIndex paper's reference following capability,
/// allowing the retrieval system to follow "see Appendix G" style references.
#[derive(Debug, Clone)]
pub struct ReferenceFollower {
    config: ReferenceConfig,
}

impl Default for ReferenceFollower {
    fn default() -> Self {
        Self::new(ReferenceConfig::default())
    }
}

impl ReferenceFollower {
    /// Create a new reference follower with configuration.
    pub fn new(config: ReferenceConfig) -> Self {
        Self { config }
    }

    /// Create with default configuration.
    pub fn with_defaults() -> Self {
        Self::default()
    }

    /// Follow all references from a node's content.
    ///
    /// Returns a list of followed references with their resolved targets.
    pub fn follow_from_node(
        &self,
        tree: &DocumentTree,
        index: &RetrievalIndex,
        node_id: NodeId,
    ) -> Vec<FollowedReference> {
        let mut results = Vec::new();
        let mut visited = HashSet::new();
        visited.insert(node_id);

        self.follow_from_node_inner(tree, index, node_id, 0, &mut visited, &mut results);

        // Sort by confidence and limit
        results.sort_by(|a, b| {
            b.reference
                .confidence
                .partial_cmp(&a.reference.confidence)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        results.truncate(self.config.max_references);

        results
    }

    fn follow_from_node_inner(
        &self,
        tree: &DocumentTree,
        index: &RetrievalIndex,
        node_id: NodeId,
        depth: usize,
        visited: &mut HashSet<NodeId>,
        results: &mut Vec<FollowedReference>,
    ) {
        if depth >= self.config.max_depth {
            return;
        }

        if results.len() >= self.config.max_references {
            return;
        }

        // Get node content
        let node = match tree.get(node_id) {
            Some(n) => n,
            None => return,
        };

        // Use pre-extracted references if available, otherwise extract
        let refs = if !node.references.is_empty() {
            node.references.clone()
        } else {
            ReferenceExtractor::extract(&node.content)
        };

        // Resolve references
        let resolved_refs = ReferenceExtractor::extract_and_resolve(&node.content, tree, index);

        for r#ref in resolved_refs {
            // Check if we should follow this type
            if !self.config.should_follow(r#ref.ref_type) {
                continue;
            }

            // Check confidence
            if r#ref.confidence < self.config.min_confidence {
                continue;
            }

            let followed = FollowedReference {
                source_node: node_id,
                reference: r#ref.clone(),
                target_node: r#ref.target_node,
                depth,
            };

            results.push(followed);

            // Recursively follow if resolved and not visited
            if let Some(target_id) = r#ref.target_node {
                if !visited.contains(&target_id) {
                    visited.insert(target_id);
                    self.follow_from_node_inner(tree, index, target_id, depth + 1, visited, results);
                }
            }
        }
    }

    /// Follow references from multiple nodes.
    ///
    /// Useful for expanding content after initial search.
    pub fn follow_from_nodes(
        &self,
        tree: &DocumentTree,
        index: &RetrievalIndex,
        node_ids: &[NodeId],
    ) -> Vec<FollowedReference> {
        let mut all_results = Vec::new();
        let mut visited = HashSet::new();
        visited.extend(node_ids.iter().copied());

        for &node_id in node_ids {
            self.follow_from_node_inner(tree, index, node_id, 0, &mut visited, &mut all_results);
        }

        // Deduplicate by target node
        let mut seen_targets = HashSet::new();
        all_results.retain(|r| {
            if let Some(target) = r.target_node {
                seen_targets.insert(target)
            } else {
                true // Keep unresolved references
            }
        });

        // Sort and limit
        all_results.sort_by(|a, b| {
            b.reference
                .confidence
                .partial_cmp(&a.reference.confidence)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        all_results.truncate(self.config.max_references);

        all_results
    }

    /// Find all nodes reachable via references from a starting node.
    ///
    /// Returns a set of node IDs that can be reached by following references.
    pub fn find_reachable_nodes(
        &self,
        tree: &DocumentTree,
        index: &RetrievalIndex,
        start_node: NodeId,
    ) -> HashSet<NodeId> {
        let mut reachable = HashSet::new();
        let mut stack = vec![start_node];

        while let Some(node_id) = stack.pop() {
            if reachable.contains(&node_id) {
                continue;
            }
            reachable.insert(node_id);

            // Get references from this node
            if let Some(node) = tree.get(node_id) {
                let refs = if !node.references.is_empty() {
                    node.references.clone()
                } else {
                    ReferenceExtractor::extract(&node.content)
                };

                // Resolve and add targets to stack
                let resolved = ReferenceExtractor::extract_and_resolve(&node.content, tree, index);
                for r#ref in resolved {
                    if self.config.should_follow(r#ref.ref_type)
                        && r#ref.confidence >= self.config.min_confidence
                    {
                        if let Some(target_id) = r#ref.target_node {
                            if !reachable.contains(&target_id) {
                                stack.push(target_id);
                            }
                        }
                    }
                }
            }

            // Limit exploration
            if reachable.len() >= self.config.max_references * 2 {
                break;
            }
        }

        reachable
    }

    /// Get the configuration.
    pub fn config(&self) -> &ReferenceConfig {
        &self.config
    }
}

/// Reference expansion result for content aggregation.
#[derive(Debug, Clone)]
pub struct ReferenceExpansion {
    /// Original node IDs.
    pub original_nodes: Vec<NodeId>,
    /// Expanded node IDs (via references).
    pub expanded_nodes: Vec<NodeId>,
    /// References that were followed.
    pub references: Vec<FollowedReference>,
    /// Total expansion depth.
    pub depth: usize,
}

impl ReferenceExpansion {
    /// Get all nodes (original + expanded).
    pub fn all_nodes(&self) -> Vec<NodeId> {
        let mut all = self.original_nodes.clone();
        all.extend(self.expanded_nodes.iter().copied());
        all
    }

    /// Get only the expanded nodes.
    pub fn new_nodes(&self) -> &[NodeId] {
        &self.expanded_nodes
    }

    /// Check if any references were followed.
    pub fn has_expansion(&self) -> bool {
        !self.expanded_nodes.is_empty()
    }
}

/// Expand search results by following references.
///
/// This is a convenience function that combines search results with
/// reference following.
pub fn expand_with_references(
    tree: &DocumentTree,
    index: &RetrievalIndex,
    initial_nodes: &[NodeId],
    config: Option<ReferenceConfig>,
) -> ReferenceExpansion {
    let config = config.unwrap_or_default();
    let follower = ReferenceFollower::new(config);

    let references = follower.follow_from_nodes(tree, index, initial_nodes);

    // Collect expanded nodes
    let mut expanded_nodes = Vec::new();
    let mut seen = HashSet::new();
    seen.extend(initial_nodes.iter().copied());

    for r#ref in &references {
        if let Some(target_id) = r#ref.target_node {
            if !seen.contains(&target_id) {
                seen.insert(target_id);
                expanded_nodes.push(target_id);
            }
        }
    }

    // Calculate max depth
    let depth = references.iter().map(|r| r.depth).max().unwrap_or(0);

    ReferenceExpansion {
        original_nodes: initial_nodes.to_vec(),
        expanded_nodes,
        references,
        depth,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reference_config_default() {
        let config = ReferenceConfig::default();
        assert_eq!(config.max_depth, 3);
        assert_eq!(config.max_references, 10);
        assert!(config.follow_pages);
        assert!(config.follow_tables_figures);
    }

    #[test]
    fn test_reference_config_conservative() {
        let config = ReferenceConfig::conservative();
        assert_eq!(config.max_depth, 2);
        assert_eq!(config.max_references, 5);
    }

    #[test]
    fn test_reference_config_aggressive() {
        let config = ReferenceConfig::aggressive();
        assert_eq!(config.max_depth, 5);
        assert_eq!(config.max_references, 20);
    }

    #[test]
    fn test_reference_config_should_follow() {
        let config = ReferenceConfig::default();

        assert!(config.should_follow(RefType::Section));
        assert!(config.should_follow(RefType::Appendix));
        assert!(config.should_follow(RefType::Table));
        assert!(config.should_follow(RefType::Page));
        assert!(!config.should_follow(RefType::Unknown));
    }

    #[test]
    fn test_followed_reference_is_resolved() {
        use indextree::Arena;

        let mut arena = Arena::new();
        let node = arena.new_node(crate::document::TreeNode::default());
        let node_id = NodeId(node);

        let resolved = FollowedReference {
            source_node: node_id,
            reference: NodeReference::new(
                "Section 2.1".to_string(),
                "2.1".to_string(),
                RefType::Section,
                0,
            ),
            target_node: Some(node_id),
            depth: 0,
        };

        let unresolved = FollowedReference {
            source_node: node_id,
            reference: NodeReference::new(
                "Section 99".to_string(),
                "99".to_string(),
                RefType::Section,
                0,
            ),
            target_node: None,
            depth: 0,
        };

        assert!(resolved.is_resolved());
        assert!(!unresolved.is_resolved());
    }

    #[test]
    fn test_reference_expansion() {
        let expansion = ReferenceExpansion {
            original_nodes: vec![],
            expanded_nodes: vec![],
            references: vec![],
            depth: 0,
        };

        assert!(!expansion.has_expansion());
        assert_eq!(expansion.all_nodes().len(), 0);
    }
}
