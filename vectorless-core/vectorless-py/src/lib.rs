// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Python bindings for vectorless.

use pyo3::prelude::*;

mod answer;
mod config;
mod document;
mod engine;
mod error;
mod graph;
mod metrics;

use answer::{PyAnswer, PyEvidence, PyReasoningTrace, PyTraceStep};
use config::PyConfig;
use document::{
    PyCollectedEvidence, PyConcept, PyDocument, PyDocumentInfo, PyFindResult, PyMatchResult,
    PyNodeInfo, PyNodeStats, PySectionSummary, PySimilarResult, PyTocEntry, PyTopicEntry,
    PyWordCount,
};
use engine::PyEngine;
use error::VectorlessError;
use graph::{PyDocumentGraph, PyDocumentGraphNode, PyEdgeEvidence, PyGraphEdge, PyWeightedKeyword};
use metrics::{PyLlmMetricsReport, PyMetricsReport, PyRetrievalMetricsReport};

/// Vectorless — Document Understanding Engine for AI.
///
/// ```python
/// from vectorless import Engine
///
/// engine = Engine(api_key="sk-...", model="gpt-4o")
/// doc = await engine.ingest("./report.pdf")
/// answer = await engine.ask("What is the revenue?", doc_ids=[doc.doc_id])
/// print(answer.content)
/// ```
#[pymodule]
fn _vectorless(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<VectorlessError>()?;
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
    m.add_class::<PyAnswer>()?;
    m.add_class::<PyEvidence>()?;
    m.add_class::<PyReasoningTrace>()?;
    m.add_class::<PyTraceStep>()?;
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
