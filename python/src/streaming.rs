// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! PyO3 streaming query wrapper.
//!
//! Bridges Rust's `mpsc::Receiver<RetrieveEvent>` to a Python async iterator,
//! yielding real-time retrieval progress events as dicts.

use pyo3::exceptions::PyStopAsyncIteration;
use pyo3::prelude::*;
use pyo3::types::PyDict;
use pyo3_async_runtimes::tokio::future_into_py;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};

use ::vectorless::retrieval::{RetrieveEvent, SufficiencyLevel};

/// Convert a `RetrieveEvent` into a Python dict with a `"type"` key.
fn event_to_dict(event: RetrieveEvent, py: Python<'_>) -> PyObject {
    let dict = PyDict::new(py);
    match event {
        RetrieveEvent::Started { query, strategy } => {
            dict.set_item("type", "started").unwrap();
            dict.set_item("query", query).unwrap();
            dict.set_item("strategy", strategy).unwrap();
        }
        RetrieveEvent::StageCompleted { stage, elapsed_ms } => {
            dict.set_item("type", "stage_completed").unwrap();
            dict.set_item("stage", stage).unwrap();
            dict.set_item("elapsed_ms", elapsed_ms).unwrap();
        }
        RetrieveEvent::NodeVisited {
            node_id,
            title,
            score,
        } => {
            dict.set_item("type", "node_visited").unwrap();
            dict.set_item("node_id", node_id).unwrap();
            dict.set_item("title", title).unwrap();
            dict.set_item("score", score).unwrap();
        }
        RetrieveEvent::ContentFound {
            node_id,
            title,
            preview,
            score,
        } => {
            dict.set_item("type", "content_found").unwrap();
            dict.set_item("node_id", node_id).unwrap();
            dict.set_item("title", title).unwrap();
            dict.set_item("preview", preview).unwrap();
            dict.set_item("score", score).unwrap();
        }
        RetrieveEvent::Backtracking { from, to, reason } => {
            dict.set_item("type", "backtracking").unwrap();
            dict.set_item("from", from).unwrap();
            dict.set_item("to", to).unwrap();
            dict.set_item("reason", reason).unwrap();
        }
        RetrieveEvent::SufficiencyCheck { level, tokens } => {
            let level_str = match level {
                SufficiencyLevel::Sufficient => "sufficient",
                SufficiencyLevel::PartialSufficient => "partial_sufficient",
                SufficiencyLevel::Insufficient => "insufficient",
            };
            dict.set_item("type", "sufficiency_check").unwrap();
            dict.set_item("level", level_str).unwrap();
            dict.set_item("tokens", tokens).unwrap();
        }
        RetrieveEvent::Completed { response } => {
            dict.set_item("type", "completed").unwrap();
            dict.set_item("confidence", response.confidence).unwrap();
            dict.set_item("is_sufficient", response.is_sufficient).unwrap();
            dict.set_item("strategy_used", response.strategy_used).unwrap();
            dict.set_item("tokens_used", response.tokens_used).unwrap();
            dict.set_item("content", response.content).unwrap();

            let results: Vec<PyObject> = response
                .results
                .into_iter()
                .map(|r| {
                    let rd = PyDict::new(py);
                    rd.set_item("node_id", &r.node_id).unwrap();
                    rd.set_item("title", &r.title).unwrap();
                    rd.set_item("content", &r.content).unwrap();
                    rd.set_item("score", r.score).unwrap();
                    rd.set_item("depth", r.depth).unwrap();
                    rd.into()
                })
                .collect();
            dict.set_item("results", results).unwrap();
        }
        RetrieveEvent::Error { message } => {
            dict.set_item("type", "error").unwrap();
            dict.set_item("message", message).unwrap();
        }
    }
    dict.into()
}

/// Python-facing async iterator over streaming retrieval events.
///
/// Usage::
///
///     stream = await engine.query_stream(ctx)
///     async for event in stream:
///         print(event["type"])
#[pyclass(name = "StreamingQuery")]
pub struct PyStreamingQuery {
    rx: Arc<Mutex<Option<mpsc::Receiver<RetrieveEvent>>>>,
}

impl PyStreamingQuery {
    pub fn new(rx: mpsc::Receiver<RetrieveEvent>) -> Self {
        Self {
            rx: Arc::new(Mutex::new(Some(rx))),
        }
    }
}

#[pymethods]
impl PyStreamingQuery {
    fn __aiter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    fn __anext__<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let rx = Arc::clone(&self.rx);
        future_into_py(py, async move {
            let mut guard = rx.lock().await;
            match guard.as_mut() {
                None => Err(PyStopAsyncIteration::new_err("stream exhausted")),
                Some(receiver) => match receiver.recv().await {
                    Some(event) => {
                        let is_terminal = matches!(
                            &event,
                            RetrieveEvent::Completed { .. } | RetrieveEvent::Error { .. }
                        );
                        if is_terminal {
                            *guard = None;
                        }
                        // Convert to Python dict — safe because future_into_py
                        // ensures we're on a thread that can acquire the GIL.
                        let obj = Python::with_gil(|py| event_to_dict(event, py));
                        Ok(obj)
                    }
                    None => {
                        *guard = None;
                        Err(PyStopAsyncIteration::new_err("stream closed"))
                    }
                },
            }
        })
    }

    fn __repr__(&self) -> String {
        "StreamingQuery(...)".to_string()
    }
}
