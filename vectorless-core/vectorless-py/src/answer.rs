// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Answer Python wrapper.

use pyo3::prelude::*;

use ::vectorless_document::Answer;

/// A reasoned answer with evidence and trace.
#[pyclass(name = "Answer", skip_from_py_object)]
pub struct PyAnswer {
    pub(crate) inner: Answer,
}

#[pymethods]
impl PyAnswer {
    /// The answer content.
    #[getter]
    fn content(&self) -> &str {
        &self.inner.content
    }

    /// Evidence supporting the answer.
    #[getter]
    fn evidence(&self) -> Vec<PyEvidence> {
        self.inner
            .evidence
            .iter()
            .map(|e| PyEvidence {
                content: e.content.clone(),
                source_path: e.source_path.clone(),
                doc_name: e.doc_name.clone(),
                relevance: e.relevance,
            })
            .collect()
    }

    /// Confidence score (0.0–1.0).
    #[getter]
    fn confidence(&self) -> f32 {
        self.inner.confidence
    }

    /// Reasoning trace — how the agent arrived at this answer.
    #[getter]
    fn trace(&self) -> PyReasoningTrace {
        PyReasoningTrace {
            steps: self
                .inner
                .trace
                .steps
                .iter()
                .map(|s| PyTraceStep {
                    action: s.action.clone(),
                    observation: s.observation.clone(),
                    round: s.round,
                })
                .collect(),
        }
    }

    fn __repr__(&self) -> String {
        format!(
            "Answer(confidence={:.2}, evidence={}, trace_steps={})",
            self.inner.confidence,
            self.inner.evidence.len(),
            self.inner.trace.steps.len()
        )
    }
}

/// A piece of evidence with source attribution.
#[pyclass(name = "Evidence", skip_from_py_object)]
pub struct PyEvidence {
    #[pyo3(get)]
    pub content: String,
    #[pyo3(get)]
    pub source_path: String,
    #[pyo3(get)]
    pub doc_name: String,
    #[pyo3(get)]
    pub relevance: f32,
}

/// Reasoning trace — always present.
#[pyclass(name = "ReasoningTrace", skip_from_py_object)]
pub struct PyReasoningTrace {
    #[pyo3(get)]
    pub steps: Vec<PyTraceStep>,
}

/// A single step in the reasoning trace.
#[pyclass(name = "TraceStep", skip_from_py_object)]
#[derive(Clone)]
pub struct PyTraceStep {
    #[pyo3(get)]
    pub action: String,
    #[pyo3(get)]
    pub observation: String,
    #[pyo3(get)]
    pub round: u32,
}
