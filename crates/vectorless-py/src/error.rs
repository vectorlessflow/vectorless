// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Python exception types and error conversion.

use pyo3::create_exception;
use pyo3::prelude::*;

use ::vectorless_engine::Error as RustError;

create_exception!(vectorless, VectorlessError, pyo3::exceptions::PyException);

/// Convert vectorless errors to Python exceptions.
pub fn to_py_err(e: RustError) -> PyErr {
    VectorlessError::new_err(e.to_string())
}
