// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Python bindings for vectorless.

use pyo3::prelude::*;

mod config;
mod document;
mod engine;
mod error;
mod graph;
mod metrics;

use config::PyConfig;
use document::{
    PyChainInfo, PyCollectedEvidence, PyConcept, PyConceptInfo, PyConceptRoute, PyDocCard,
    PyDocument, PyDocumentInfo, PyEvidenceScore, PyFindResult, PyMatchResult, PyNodeInfo,
    PyNodeRouting, PyNodeStats, PyOverlapInfo, PyRouteTarget, PySectionCard, PySectionSummary,
    PySimilarResult, PyTocEntry, PyTopicEntry, PyWordCount,
};
use engine::PyEngine;
use graph::{PyDocumentGraph, PyDocumentGraphNode, PyEdgeEvidence, PyGraphEdge, PyWeightedKeyword};
use metrics::{PyLlmMetricsReport, PyMetricsReport, PyRetrievalMetricsReport};

/// Vectorless — Document Understanding Engine for AI.
#[pymodule]
fn _vectorless(m: &Bound<'_, PyModule>) -> PyResult<()> {
    // Initialize tracing subscriber from RUST_LOG env var (once).
    use std::sync::Once;
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        tracing_subscriber::fmt()
            .with_env_filter(
                tracing_subscriber::EnvFilter::from_default_env()
                    .add_directive("vectorless=info".parse().unwrap()),
            )
            .init();
    });

    // VectorlessError is just PyRuntimeError, no need to register
    m.add_class::<PyEngine>()?;
    m.add_class::<PyDocument>()?;
    m.add_class::<PyDocumentInfo>()?;
    m.add_class::<PyConcept>()?;
    m.add_class::<PyNodeInfo>()?;
    m.add_class::<PyMatchResult>()?;
    m.add_class::<PyFindResult>()?;
    m.add_class::<PyWordCount>()?;
    m.add_class::<PyCollectedEvidence>()?;
    m.add_class::<PyTopicEntry>()?;
    m.add_class::<PySectionSummary>()?;
    m.add_class::<PyTocEntry>()?;
    m.add_class::<PyNodeStats>()?;
    m.add_class::<PySimilarResult>()?;
    m.add_class::<PySectionCard>()?;
    m.add_class::<PyDocCard>()?;
    m.add_class::<PyConceptInfo>()?;
    m.add_class::<PyRouteTarget>()?;
    m.add_class::<PyConceptRoute>()?;
    m.add_class::<PyChainInfo>()?;
    m.add_class::<PyOverlapInfo>()?;
    m.add_class::<PyEvidenceScore>()?;
    m.add_class::<PyNodeRouting>()?;
    m.add_class::<PyDocumentGraphNode>()?;
    m.add_class::<PyDocumentGraph>()?;
    m.add_class::<PyGraphEdge>()?;
    m.add_class::<PyEdgeEvidence>()?;
    m.add_class::<PyWeightedKeyword>()?;
    m.add_class::<PyLlmMetricsReport>()?;
    m.add_class::<PyRetrievalMetricsReport>()?;
    m.add_class::<PyMetricsReport>()?;
    m.add_class::<PyConfig>()?;

    m.add("__version__", env!("CARGO_PKG_VERSION"))?;

    Ok(())
}
