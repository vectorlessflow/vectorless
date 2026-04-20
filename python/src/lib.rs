// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Python bindings for vectorless.

use pyo3::prelude::*;

mod config;
mod context;
mod document;
mod engine;
mod error;
mod graph;
mod metrics;
mod results;

use config::PyConfig;
use context::{PyIndexContext, PyIndexOptions, PyQueryContext};
use document::PyDocumentInfo;
use engine::PyEngine;
use error::VectorlessError;
use graph::{PyDocumentGraph, PyDocumentGraphNode, PyEdgeEvidence, PyGraphEdge, PyWeightedKeyword};
use metrics::{PyLlmMetricsReport, PyMetricsReport, PyRetrievalMetricsReport};
use results::{
    PyEvidenceItem, PyFailedItem, PyIndexItem, PyIndexMetrics, PyIndexResult, PyQueryMetrics,
    PyQueryResult, PyQueryResultItem,
};

/// Vectorless - Reasoning-native document intelligence engine.
///
/// ```python
/// from vectorless import Engine, IndexContext, QueryContext
///
/// engine = Engine(api_key="sk-...", model="gpt-4o")
/// result = await engine.index(IndexContext.from_path("./report.pdf"))
/// answer = await engine.query(QueryContext("What is the revenue?").with_doc_ids([result.doc_id]))
/// print(answer.single().content)
/// ```
#[pymodule]
fn _vectorless(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<VectorlessError>()?;
    m.add_class::<PyIndexOptions>()?;
    m.add_class::<PyIndexContext>()?;
    m.add_class::<PyQueryContext>()?;
    m.add_class::<PyIndexResult>()?;
    m.add_class::<PyIndexItem>()?;
    m.add_class::<PyIndexMetrics>()?;
    m.add_class::<PyQueryResult>()?;
    m.add_class::<PyQueryResultItem>()?;
    m.add_class::<PyEvidenceItem>()?;
    m.add_class::<PyQueryMetrics>()?;
    m.add_class::<PyFailedItem>()?;
    m.add_class::<PyDocumentInfo>()?;
    m.add_class::<PyDocumentGraphNode>()?;
    m.add_class::<PyDocumentGraph>()?;
    m.add_class::<PyGraphEdge>()?;
    m.add_class::<PyEdgeEvidence>()?;
    m.add_class::<PyWeightedKeyword>()?;
    m.add_class::<PyLlmMetricsReport>()?;
    m.add_class::<PyRetrievalMetricsReport>()?;
    m.add_class::<PyMetricsReport>()?;
    m.add_class::<PyConfig>()?;
    m.add_class::<PyEngine>()?;

    m.add("__version__", env!("CARGO_PKG_VERSION"))?;

    Ok(())
}
