// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Pre-computed query routing table for Agent acceleration.
//!
//! Built at compile time from question hints and topic tags, the routing table
//! maps query intents and concepts to optimal entry nodes in the document tree.
//! The Agent can skip root-level exploration and navigate directly to the most
//! relevant subtree.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use super::node::NodeId;

/// A scored target node for routing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteTarget {
    /// Target node to navigate to.
    pub node_id: NodeId,
    /// Relevance score (0.0–1.0).
    pub relevance: f64,
    /// Human-readable reason for this route (e.g., "Contains Q3 revenue data").
    pub reason: String,
}

/// A concept-to-nodes mapping for semantic routing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConceptRoute {
    /// Concept name (e.g., "revenue", "authentication").
    pub concept: String,
    /// Scored target nodes for this concept.
    pub targets: Vec<RouteTarget>,
}

/// Pre-computed routing table mapping intents and concepts to entry nodes.
///
/// The Agent receives a query analysis (intent + concepts) and looks up
/// pre-computed routes to find the best navigation starting point,
/// bypassing the typical root → ls → explore cycle.
///
/// # Construction
///
/// Built by the `RoutePass` compiler pass. No LLM calls — uses existing
/// `question_hints` and `routing_keywords` from the enhance stage.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryRoutingTable {
    /// Routes derived from question hints: node → scored alternatives.
    /// Key nodes are those with `question_hints` populated by the enhance stage.
    #[serde(with = "super::serde_helpers")]
    intent_routes: HashMap<NodeId, Vec<RouteTarget>>,

    /// Routes derived from topic tags: concept → scored entry nodes.
    concept_routes: Vec<ConceptRoute>,
}

impl QueryRoutingTable {
    /// Create a new empty routing table.
    pub fn new() -> Self {
        Self {
            intent_routes: HashMap::new(),
            concept_routes: Vec::new(),
        }
    }

    /// Add an intent-based route.
    pub fn add_intent_route(&mut self, entry_node: NodeId, targets: Vec<RouteTarget>) {
        self.intent_routes.insert(entry_node, targets);
    }

    /// Add a concept-based route.
    pub fn add_concept_route(&mut self, route: ConceptRoute) {
        self.concept_routes.push(route);
    }

    /// Get intent-based routes for a specific entry node.
    pub fn intent_routes_for(&self, node_id: NodeId) -> Option<&[RouteTarget]> {
        self.intent_routes.get(&node_id).map(Vec::as_slice)
    }

    /// Get all intent route entries.
    pub fn intent_routes(&self) -> &HashMap<NodeId, Vec<RouteTarget>> {
        &self.intent_routes
    }

    /// Get all concept routes.
    pub fn concept_routes(&self) -> &[ConceptRoute] {
        &self.concept_routes
    }

    /// Look up concept routes matching a keyword.
    pub fn routes_for_concept(&self, keyword: &str) -> Vec<&RouteTarget> {
        let kw = keyword.to_lowercase();
        self.concept_routes
            .iter()
            .filter(|cr| cr.concept.to_lowercase().contains(&kw))
            .flat_map(|cr| cr.targets.iter())
            .collect()
    }

    /// Total number of intent routes.
    pub fn intent_route_count(&self) -> usize {
        self.intent_routes.len()
    }

    /// Total number of concept routes.
    pub fn concept_route_count(&self) -> usize {
        self.concept_routes.len()
    }
}

impl Default for QueryRoutingTable {
    fn default() -> Self {
        Self::new()
    }
}
