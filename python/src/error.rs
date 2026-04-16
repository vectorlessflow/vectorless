// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Python exception types and error conversion.

use pyo3::exceptions::PyException;
use pyo3::prelude::*;

use ::vectorless::error::Error as RustError;

/// Python exception for vectorless errors.
#[pyclass(extends = PyException, subclass)]
pub struct VectorlessError {
    message: String,
    kind: String,
}

#[pymethods]
impl VectorlessError {
    #[new]
    fn new_py(message: String, kind: String) -> Self {
        Self { message, kind }
    }

    #[getter]
    fn message(&self) -> &str {
        &self.message
    }

    #[getter]
    fn kind(&self) -> &str {
        &self.kind
    }

    fn __str__(&self) -> &str {
        &self.message
    }

    fn __repr__(&self) -> String {
        format!("VectorlessError('{}', kind='{}')", self.message, self.kind)
    }
}

impl VectorlessError {
    pub fn new(message: String, kind: &str) -> Self {
        Self {
            message,
            kind: kind.to_string(),
        }
    }
}

impl From<VectorlessError> for PyErr {
    fn from(err: VectorlessError) -> PyErr {
        PyErr::new::<VectorlessError, _>((err.message, err.kind))
    }
}

/// Convert vectorless errors to Python exceptions.
pub fn to_py_err(e: RustError) -> PyErr {
    let message = e.to_string();
    let kind = match &e {
        RustError::DocumentNotFound(_) => "not_found",
        RustError::Parse(_) => "parse",
        RustError::Config(_) => "config",
        RustError::Workspace(_) => "workspace",
        RustError::Llm(_) => "llm",
        _ => "unknown",
    };
    VectorlessError::new(message, kind).into()
}
