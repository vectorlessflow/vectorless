// Copyright (c) vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Reasoning chains — logical connections between document sections.
//!
//! Represents premise→conclusion relationships extracted from in-document
//! references and content structure. The Agent can follow chains to collect
//! supporting evidence across non-adjacent sections.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use super::node::NodeId;

/// Type of logical connection between sections.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChainType {
    /// A causes or leads to B.
    Causal,
    /// A provides evidence or support for B.
    Supporting,
    /// A contradicts or refutes B.
    Contradicting,
    /// B is a detailed expansion of A.
    Elaboration,
    /// A is a prerequisite step before B.
    Sequence,
}

impl ChainType {
    /// Convert to a static string label.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Causal => "causal",
            Self::Supporting => "supporting",
            Self::Contradicting => "contradicting",
            Self::Elaboration => "elaboration",
            Self::Sequence => "sequence",
        }
    }
}

/// A single reasoning chain connecting document sections.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReasoningChain {
    /// Nodes that establish the premise.
    pub premises: Vec<NodeId>,
    /// Nodes that draw conclusions.
    pub conclusions: Vec<NodeId>,
    /// Type of logical connection.
    pub chain_type: ChainType,
    /// Human-readable summary of this chain.
    pub summary: String,
}

/// Index of reasoning chains with bidirectional node lookup.
///
/// Allows the Agent to find all chains involving a specific node,
/// enabling "follow the reasoning" navigation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChainIndex {
    /// All reasoning chains.
    pub chains: Vec<ReasoningChain>,
    /// Node → indices of chains that involve this node.
    #[serde(with = "super::serde_helpers")]
    node_chains: HashMap<NodeId, Vec<usize>>,
}

impl ChainIndex {
    /// Create a new empty chain index.
    pub fn new() -> Self {
        Self {
            chains: Vec::new(),
            node_chains: HashMap::new(),
        }
    }

    /// Add a chain and update the node index.
    pub fn add_chain(&mut self, chain: ReasoningChain) {
        let idx = self.chains.len();
        for node_id in chain.premises.iter().chain(chain.conclusions.iter()) {
            self.node_chains.entry(*node_id).or_default().push(idx);
        }
        self.chains.push(chain);
    }

    /// Get all chains involving a specific node.
    pub fn chains_for(&self, node_id: NodeId) -> Vec<&ReasoningChain> {
        match self.node_chains.get(&node_id) {
            Some(indices) => indices.iter().filter_map(|&i| self.chains.get(i)).collect(),
            None => Vec::new(),
        }
    }

    /// Get chains where the given node is a premise.
    pub fn premises_from(&self, node_id: NodeId) -> Vec<&ReasoningChain> {
        self.chains_for(node_id)
            .into_iter()
            .filter(|c| c.premises.contains(&node_id))
            .collect()
    }

    /// Get chains where the given node is a conclusion.
    pub fn conclusions_from(&self, node_id: NodeId) -> Vec<&ReasoningChain> {
        self.chains_for(node_id)
            .into_iter()
            .filter(|c| c.conclusions.contains(&node_id))
            .collect()
    }

    /// Total number of chains.
    pub fn chain_count(&self) -> usize {
        self.chains.len()
    }

    /// Total number of nodes involved in chains.
    pub fn node_count(&self) -> usize {
        self.node_chains.len()
    }
}

impl Default for ChainIndex {
    fn default() -> Self {
        Self::new()
    }
}
