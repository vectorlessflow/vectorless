// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! DocumentGraph Python wrappers.

use pyo3::prelude::*;

use ::vectorless_engine::{
    DocumentGraph, DocumentGraphNode, EdgeEvidence, GraphEdge, WeightedKeyword,
};

/// A keyword with weight from document analysis.
#[pyclass(name = "WeightedKeyword")]
pub struct PyWeightedKeyword {
    pub(crate) inner: WeightedKeyword,
}

#[pymethods]
impl PyWeightedKeyword {
    #[getter]
    fn keyword(&self) -> &str {
        &self.inner.keyword
    }

    #[getter]
    fn weight(&self) -> f32 {
        self.inner.weight
    }

    fn __repr__(&self) -> String {
        format!(
            "WeightedKeyword('{}', weight={:.2})",
            self.inner.keyword, self.inner.weight
        )
    }
}

/// Evidence for a cross-document connection.
#[pyclass(name = "EdgeEvidence")]
pub struct PyEdgeEvidence {
    pub(crate) inner: EdgeEvidence,
}

#[pymethods]
impl PyEdgeEvidence {
    /// Number of shared keywords.
    #[getter]
    fn shared_keyword_count(&self) -> usize {
        self.inner.shared_keyword_count
    }

    /// Jaccard similarity of keyword sets.
    #[getter]
    fn keyword_jaccard(&self) -> f32 {
        self.inner.keyword_jaccard
    }

    /// Shared keywords with weights.
    #[getter]
    fn shared_keywords(&self) -> Vec<(String, f32, f32)> {
        self.inner
            .shared_keywords
            .iter()
            .map(|sk| (sk.keyword.clone(), sk.source_weight, sk.target_weight))
            .collect()
    }

    fn __repr__(&self) -> String {
        format!(
            "EdgeEvidence(shared={}, jaccard={:.2})",
            self.inner.shared_keyword_count, self.inner.keyword_jaccard
        )
    }
}

/// An edge representing a relationship between two documents.
#[pyclass(name = "GraphEdge")]
pub struct PyGraphEdge {
    pub(crate) inner: GraphEdge,
}

#[pymethods]
impl PyGraphEdge {
    /// Target document ID.
    #[getter]
    fn target_doc_id(&self) -> &str {
        &self.inner.target_doc_id
    }

    /// Edge weight (connection strength).
    #[getter]
    fn weight(&self) -> f32 {
        self.inner.weight
    }

    /// Evidence for this connection.
    #[getter]
    fn evidence(&self) -> PyEdgeEvidence {
        PyEdgeEvidence {
            inner: self.inner.evidence.clone(),
        }
    }

    fn __repr__(&self) -> String {
        format!(
            "GraphEdge(target='{}', weight={:.2})",
            self.inner.target_doc_id, self.inner.weight
        )
    }
}

/// A node in the document graph representing an indexed document.
#[pyclass(name = "DocumentGraphNode")]
pub struct PyDocumentGraphNode {
    pub(crate) inner: DocumentGraphNode,
}

#[pymethods]
impl PyDocumentGraphNode {
    #[getter]
    fn doc_id(&self) -> &str {
        &self.inner.doc_id
    }

    #[getter]
    fn title(&self) -> &str {
        &self.inner.title
    }

    #[getter]
    fn format(&self) -> &str {
        &self.inner.format
    }

    #[getter]
    fn node_count(&self) -> usize {
        self.inner.node_count
    }

    /// Top keywords extracted from the document.
    #[getter]
    fn top_keywords(&self) -> Vec<PyWeightedKeyword> {
        self.inner
            .top_keywords
            .iter()
            .map(|kw| PyWeightedKeyword { inner: kw.clone() })
            .collect()
    }

    fn __repr__(&self) -> String {
        format!(
            "DocumentGraphNode(doc_id='{}', title='{}')",
            self.inner.doc_id, self.inner.title
        )
    }
}

/// Cross-document relationship graph.
///
/// Automatically rebuilt after indexing. Connects documents
/// that share keywords via Jaccard similarity.
#[pyclass(name = "DocumentGraph")]
pub struct PyDocumentGraph {
    pub(crate) inner: DocumentGraph,
}

#[pymethods]
impl PyDocumentGraph {
    /// Number of document nodes.
    fn node_count(&self) -> usize {
        self.inner.node_count()
    }

    /// Number of relationship edges.
    fn edge_count(&self) -> usize {
        self.inner.edge_count()
    }

    /// Get a document node by ID.
    fn get_node(&self, doc_id: String) -> Option<PyDocumentGraphNode> {
        self.inner
            .get_node(&doc_id)
            .map(|n| PyDocumentGraphNode { inner: n.clone() })
    }

    /// Get all document IDs in the graph.
    fn doc_ids(&self) -> Vec<String> {
        self.inner.doc_ids().map(|s| s.to_string()).collect()
    }

    /// Get edges (neighbors) for a document.
    fn get_neighbors(&self, doc_id: String) -> Vec<PyGraphEdge> {
        self.inner
            .get_neighbors(&doc_id)
            .iter()
            .map(|e| PyGraphEdge { inner: e.clone() })
            .collect()
    }

    /// Whether the graph is empty.
    fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    fn __repr__(&self) -> String {
        format!(
            "DocumentGraph(nodes={}, edges={})",
            self.inner.node_count(),
            self.inner.edge_count()
        )
    }
}
