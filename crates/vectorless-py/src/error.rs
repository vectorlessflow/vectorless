// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Python exception types and error conversion.

use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;

use ::vectorless_engine::Error as RustError;

/// Convert vectorless errors to Python exceptions.
pub fn to_py_err(e: RustError) -> PyErr {
    PyRuntimeError::new_err(e.to_string())
}
