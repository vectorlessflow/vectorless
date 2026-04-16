// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! DocumentInfo Python wrapper.

use pyo3::prelude::*;

use ::vectorless::client::DocumentInfo;

/// Information about an indexed document.
#[pyclass(name = "DocumentInfo")]
pub struct PyDocumentInfo {
    pub(crate) inner: DocumentInfo,
}

#[pymethods]
impl PyDocumentInfo {
    #[getter]
    fn id(&self) -> &str {
        &self.inner.id
    }

    #[getter]
    fn name(&self) -> &str {
        &self.inner.name
    }

    #[getter]
    fn format(&self) -> &str {
        &self.inner.format
    }

    #[getter]
    fn description(&self) -> Option<&str> {
        self.inner.description.as_deref()
    }

    #[getter]
    fn source_path(&self) -> Option<&str> {
        self.inner.source_path.as_deref()
    }

    #[getter]
    fn page_count(&self) -> Option<usize> {
        self.inner.page_count
    }

    #[getter]
    fn line_count(&self) -> Option<usize> {
        self.inner.line_count
    }

    fn __repr__(&self) -> String {
        format!(
            "DocumentInfo(id='{}', name='{}', format='{}')",
            self.inner.id, self.inner.name, self.inner.format
        )
    }
}
