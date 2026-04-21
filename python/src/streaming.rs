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
use tokio::sync::{Mutex, mpsc};

use ::vectorless::{RetrieveEvent, SufficiencyLevel};

/// Convert a `RetrieveEvent` into a Python dict with a `"type"` key.
fn event_to_dict(event: RetrieveEvent, py: Python<'_>) -> PyResult<Bound<'_, PyDict>> {
    let dict = PyDict::new(py);
    match event {
        RetrieveEvent::Started { query, strategy } => {
            dict.set_item("type", "started")?;
            dict.set_item("query", query)?;
            dict.set_item("strategy", strategy)?;
        }
        RetrieveEvent::StageCompleted { stage, elapsed_ms } => {
            dict.set_item("type", "stage_completed")?;
            dict.set_item("stage", stage)?;
            dict.set_item("elapsed_ms", elapsed_ms)?;
        }
        RetrieveEvent::NodeVisited {
            node_id,
            title,
            score,
        } => {
            dict.set_item("type", "node_visited")?;
            dict.set_item("node_id", node_id)?;
            dict.set_item("title", title)?;
            dict.set_item("score", score)?;
        }
        RetrieveEvent::ContentFound {
            node_id,
            title,
            preview,
            score,
        } => {
            dict.set_item("type", "content_found")?;
            dict.set_item("node_id", node_id)?;
            dict.set_item("title", title)?;
            dict.set_item("preview", preview)?;
            dict.set_item("score", score)?;
        }
        RetrieveEvent::Backtracking { from, to, reason } => {
            dict.set_item("type", "backtracking")?;
            dict.set_item("from", from)?;
            dict.set_item("to", to)?;
            dict.set_item("reason", reason)?;
        }
        RetrieveEvent::SufficiencyCheck { level, tokens } => {
            let level_str = match level {
                SufficiencyLevel::Sufficient => "sufficient",
                SufficiencyLevel::PartialSufficient => "partial_sufficient",
                SufficiencyLevel::Insufficient => "insufficient",
            };
            dict.set_item("type", "sufficiency_check")?;
            dict.set_item("level", level_str)?;
            dict.set_item("tokens", tokens)?;
        }
        RetrieveEvent::Completed { response } => {
            dict.set_item("type", "completed")?;
            dict.set_item("confidence", response.confidence)?;
            dict.set_item("is_sufficient", response.is_sufficient)?;
            dict.set_item("strategy_used", response.strategy_used)?;
            dict.set_item("tokens_used", response.tokens_used)?;
            dict.set_item("content", response.content)?;

            let results: Vec<Bound<'_, PyDict>> = response
                .results
                .into_iter()
                .map(|r| {
                    let rd = PyDict::new(py);
                    rd.set_item("node_id", &r.node_id)?;
                    rd.set_item("title", &r.title)?;
                    rd.set_item("content", &r.content)?;
                    rd.set_item("score", r.score)?;
                    rd.set_item("depth", r.depth)?;
                    Ok(rd)
                })
                .collect::<PyResult<Vec<_>>>()?;
            dict.set_item("results", results)?;
        }
        RetrieveEvent::Error { message } => {
            dict.set_item("type", "error")?;
            dict.set_item("message", message)?;
        }
    }
    Ok(dict)
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
        let rx: Arc<Mutex<Option<mpsc::Receiver<RetrieveEvent>>>> = Arc::clone(&self.rx);
        future_into_py(py, async move {
            let mut guard = rx.lock().await;
            let receiver: &mut Option<mpsc::Receiver<RetrieveEvent>> = &mut *guard;
            match receiver {
                None => Err(PyStopAsyncIteration::new_err("stream exhausted")),
                Some(rx) => match rx.recv().await {
                    Some(event) => {
                        let is_terminal = matches!(
                            &event,
                            RetrieveEvent::Completed { .. } | RetrieveEvent::Error { .. }
                        );
                        if is_terminal {
                            *guard = None;
                        }
                        // We cannot convert to dict here (no Python token in async context).
                        // Instead, store the event and convert on the Python side.
                        // PyO3 0.28: future_into_py resolves on the Python thread,
                        // so we use Python::with_gil equivalent via pyo3_async_runtimes.
                        //
                        // The cleanest approach: wrap in a PyO3-compatible type.
                        // Since RetrieveEvent doesn't implement IntoPyObject, we convert
                        // to a simple serializable form.
                        Ok(SerializedEvent(event))
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

/// Wrapper to carry a RetrieveEvent across the async boundary
/// and convert it to a dict on the Python thread.
struct SerializedEvent(RetrieveEvent);

impl<'py> IntoPyObject<'py> for SerializedEvent {
    type Target = PyDict;
    type Output = Bound<'py, Self::Target>;
    type Error = PyErr;

    fn into_pyobject(self, py: Python<'py>) -> Result<Self::Output, Self::Error> {
        event_to_dict(self.0, py)
    }
}
