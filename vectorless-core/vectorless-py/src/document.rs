// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! DocumentInfo Python wrapper.

use pyo3::prelude::*;

use ::vectorless::DocumentInfo;

/// Information about an understood document.
#[pyclass(name = "DocumentInfo")]
pub struct PyDocumentInfo {
    pub(crate) inner: DocumentInfo,
}

#[pymethods]
impl PyDocumentInfo {
    #[getter]
    fn doc_id(&self) -> &str {
        &self.inner.doc_id
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
    fn summary(&self) -> &str {
        &self.inner.summary
    }

    #[getter]
    fn concepts(&self) -> Vec<PyConcept> {
        self.inner
            .concepts
            .iter()
            .map(|c| PyConcept {
                name: c.name.clone(),
                summary: c.summary.clone(),
                sections: c.sections.clone(),
            })
            .collect()
    }

    #[getter]
    fn section_count(&self) -> usize {
        self.inner.section_count
    }

    #[getter]
    fn page_count(&self) -> Option<usize> {
        self.inner.page_count
    }

    fn __repr__(&self) -> String {
        format!(
            "DocumentInfo(doc_id='{}', name='{}', format='{}')",
            self.inner.doc_id, self.inner.name, self.inner.format
        )
    }
}

/// A key concept extracted from a document.
#[pyclass(name = "Concept")]
pub struct PyConcept {
    #[pyo3(get)]
    pub name: String,
    #[pyo3(get)]
    pub summary: String,
    #[pyo3(get)]
    pub sections: Vec<String>,
}
