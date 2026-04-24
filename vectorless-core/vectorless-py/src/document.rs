// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Document types for Python bindings.

use std::sync::Arc;

use pyo3::prelude::*;
use pyo3_async_runtimes::tokio::future_into_py;
use tokio::sync::Mutex;

use vectorless_primitives::{
    CollectedEvidence, ConceptInfo, DocCardInfo, DocumentNavigator, FindResult, MatchResult,
    NodeInfo, NodeStats, SectionCardInfo, SectionSummaryInfo, SimilarResult, TocEntry,
    TopicEntryInfo, WordCount,
};

use super::error::VectorlessError;

// =========================================================================
// PyDocumentInfo (existing — returned by ingest)
// =========================================================================

/// Information about an understood document.
#[pyclass(name = "DocumentInfo", skip_from_py_object)]
pub struct PyDocumentInfo {
    pub(crate) inner: vectorless_engine::DocumentInfo,
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
#[pyclass(name = "Concept", skip_from_py_object)]
pub struct PyConcept {
    #[pyo3(get)]
    pub name: String,
    #[pyo3(get)]
    pub summary: String,
    #[pyo3(get)]
    pub sections: Vec<String>,
}

// =========================================================================
// PyDocument — full navigable document
// =========================================================================

/// A navigable document with cursor state, evidence collection, and search.
///
/// All methods are **async** — use `await` to call them.
///
/// ```python
/// doc = await engine.load_document(doc_id)
/// children = await doc.ls()
/// await doc.cd(children[0].id)
/// print(await doc.pwd())
/// print(await doc.cat(None))
/// ```
#[pyclass(name = "Document", skip_from_py_object)]
pub struct PyDocument {
    inner: Arc<Mutex<DocumentNavigator>>,
}

impl PyDocument {
    /// Create a PyDocument from a DocumentNavigator.
    pub fn from_navigator(nav: DocumentNavigator) -> Self {
        Self {
            inner: Arc::new(Mutex::new(nav)),
        }
    }
}

// Helper: convert u64 id to Python string "n{id}"
fn id_to_str(id: u64) -> String {
    format!("n{id}")
}

fn to_py_err(e: impl std::fmt::Display) -> PyErr {
    PyErr::from(VectorlessError::new(e.to_string(), "navigation"))
}

#[pymethods]
impl PyDocument {
    // ── Navigation ──────────────────────────────────────────────────────

    /// List children of the current node.
    fn ls<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let nav = Arc::clone(&self.inner);
        future_into_py(py, async move {
            let nav = nav.lock().await;
            let result = nav.ls().await;
            Ok(result.into_iter().map(PyNodeInfo::from).collect::<Vec<_>>())
        })
    }

    /// Navigate to a node by its id string (e.g., "n42").
    fn cd<'py>(&self, py: Python<'py>, node_id: String) -> PyResult<Bound<'py, PyAny>> {
        let nav = Arc::clone(&self.inner);
        future_into_py(py, async move {
            let mut nav = nav.lock().await;
            nav.cd(&node_id).await.map_err(to_py_err)
        })
    }

    /// Navigate to a child by title (fuzzy matching).
    fn cd_by_title<'py>(&self, py: Python<'py>, title: String) -> PyResult<Bound<'py, PyAny>> {
        let nav = Arc::clone(&self.inner);
        future_into_py(py, async move {
            let mut nav = nav.lock().await;
            nav.cd_by_title(&title).await.map_err(to_py_err)
        })
    }

    /// Navigate up to the parent node.
    fn cd_up<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let nav = Arc::clone(&self.inner);
        future_into_py(py, async move {
            let mut nav = nav.lock().await;
            nav.cd_up().await.map_err(to_py_err)
        })
    }

    /// Navigate back to the root node.
    fn cd_root<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let nav = Arc::clone(&self.inner);
        future_into_py(py, async move {
            let mut nav = nav.lock().await;
            nav.cd_root().await;
            Ok(())
        })
    }

    /// Return the current navigation path.
    fn pwd<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let nav = Arc::clone(&self.inner);
        future_into_py(py, async move {
            let nav = nav.lock().await;
            Ok(nav.pwd().await)
        })
    }

    // ── Content ─────────────────────────────────────────────────────────

    /// Read node content and collect as evidence. None = current node.
    #[pyo3(signature = (node_id=None))]
    fn cat<'py>(&self, py: Python<'py>, node_id: Option<String>) -> PyResult<Bound<'py, PyAny>> {
        let nav = Arc::clone(&self.inner);
        future_into_py(py, async move {
            let mut nav = nav.lock().await;
            nav.cat(node_id.as_deref()).await.map_err(to_py_err)
        })
    }

    /// Regex search across the current subtree. Returns up to 30 matches.
    fn grep<'py>(&self, py: Python<'py>, pattern: String) -> PyResult<Bound<'py, PyAny>> {
        let nav = Arc::clone(&self.inner);
        future_into_py(py, async move {
            let nav = nav.lock().await;
            nav.grep(&pattern)
                .await
                .map(|r| r.into_iter().map(PyMatchResult::from).collect::<Vec<_>>())
                .map_err(to_py_err)
        })
    }

    /// Search for nodes by keyword in title or content (case-insensitive).
    fn find<'py>(&self, py: Python<'py>, keyword: String) -> PyResult<Bound<'py, PyAny>> {
        let nav = Arc::clone(&self.inner);
        future_into_py(py, async move {
            let nav = nav.lock().await;
            Ok(nav
                .find(&keyword)
                .await
                .into_iter()
                .map(PyFindResult::from)
                .collect::<Vec<_>>())
        })
    }

    /// Preview the first N lines of a node without collecting evidence.
    #[pyo3(signature = (node_id=None, n=10))]
    fn head<'py>(
        &self,
        py: Python<'py>,
        node_id: Option<String>,
        n: usize,
    ) -> PyResult<Bound<'py, PyAny>> {
        let nav = Arc::clone(&self.inner);
        future_into_py(py, async move {
            let nav = nav.lock().await;
            nav.head(node_id.as_deref(), n).await.map_err(to_py_err)
        })
    }

    /// Count lines, words, and characters in a node's content.
    #[pyo3(signature = (node_id=None))]
    fn wc<'py>(&self, py: Python<'py>, node_id: Option<String>) -> PyResult<Bound<'py, PyAny>> {
        let nav = Arc::clone(&self.inner);
        future_into_py(py, async move {
            let nav = nav.lock().await;
            nav.wc(node_id.as_deref())
                .await
                .map(PyWordCount::from)
                .map_err(to_py_err)
        })
    }

    // ── Metadata ────────────────────────────────────────────────────────

    /// Document-level summary.
    fn summary<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let nav = Arc::clone(&self.inner);
        future_into_py(py, async move {
            let nav = nav.lock().await;
            Ok(nav.summary().await.to_string())
        })
    }

    /// Number of sections in the tree.
    fn section_count<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let nav = Arc::clone(&self.inner);
        future_into_py(py, async move {
            let nav = nav.lock().await;
            Ok(nav.section_count().await)
        })
    }

    /// Document ID.
    fn doc_id<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let nav = Arc::clone(&self.inner);
        future_into_py(py, async move {
            let nav = nav.lock().await;
            Ok(nav.doc_id().await.to_string())
        })
    }

    /// Document name.
    fn doc_name<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let nav = Arc::clone(&self.inner);
        future_into_py(py, async move {
            let nav = nav.lock().await;
            Ok(nav.doc_name().await.to_string())
        })
    }

    // ── Reasoning Index ─────────────────────────────────────────────────

    /// Look up topic entries for a keyword.
    fn keyword_entries<'py>(
        &self,
        py: Python<'py>,
        keyword: String,
    ) -> PyResult<Bound<'py, PyAny>> {
        let nav = Arc::clone(&self.inner);
        future_into_py(py, async move {
            let nav = nav.lock().await;
            Ok(nav
                .keyword_entries(&keyword)
                .await
                .into_iter()
                .map(PyTopicEntry::from)
                .collect::<Vec<_>>())
        })
    }

    /// Section summaries from the reasoning index.
    fn topic_summary<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let nav = Arc::clone(&self.inner);
        future_into_py(py, async move {
            let nav = nav.lock().await;
            Ok(nav
                .topic_summary()
                .await
                .into_iter()
                .map(PySectionSummary::from)
                .collect::<Vec<_>>())
        })
    }

    /// Find sections related to any of the given keywords.
    fn related_sections<'py>(
        &self,
        py: Python<'py>,
        keywords: Vec<String>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let nav = Arc::clone(&self.inner);
        future_into_py(py, async move {
            let nav = nav.lock().await;
            Ok(nav
                .related_sections(&keywords)
                .await
                .into_iter()
                .map(id_to_str)
                .collect::<Vec<_>>())
        })
    }

    // ── Evidence ────────────────────────────────────────────────────────

    /// Explicitly collect evidence from a node.
    fn collect_evidence<'py>(
        &self,
        py: Python<'py>,
        node_id: String,
    ) -> PyResult<Bound<'py, PyAny>> {
        let nav = Arc::clone(&self.inner);
        future_into_py(py, async move {
            let mut nav = nav.lock().await;
            nav.collect_evidence(&node_id).await.map_err(to_py_err)
        })
    }

    /// Return all collected evidence.
    fn evidence<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let nav = Arc::clone(&self.inner);
        future_into_py(py, async move {
            let nav = nav.lock().await;
            Ok(nav
                .evidence()
                .await
                .iter()
                .cloned()
                .map(PyCollectedEvidence::from)
                .collect::<Vec<_>>())
        })
    }

    /// Clear all collected evidence.
    fn clear_evidence<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let nav = Arc::clone(&self.inner);
        future_into_py(py, async move {
            let mut nav = nav.lock().await;
            nav.clear_evidence().await;
            Ok(())
        })
    }

    // ── Tree inspection ─────────────────────────────────────────────────

    /// Root node id.
    fn root_id<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let nav = Arc::clone(&self.inner);
        future_into_py(py, async move {
            let nav = nav.lock().await;
            Ok(id_to_str(nav.root_id().await))
        })
    }

    /// Current cursor node id.
    fn current_id<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let nav = Arc::clone(&self.inner);
        future_into_py(py, async move {
            let nav = nav.lock().await;
            Ok(id_to_str(nav.current_id().await))
        })
    }

    /// List children of an arbitrary node.
    fn children_of<'py>(&self, py: Python<'py>, node_id: String) -> PyResult<Bound<'py, PyAny>> {
        let nav = Arc::clone(&self.inner);
        future_into_py(py, async move {
            let nav = nav.lock().await;
            nav.children_of(&node_id)
                .await
                .map(|r| r.into_iter().map(PyNodeInfo::from).collect::<Vec<_>>())
                .map_err(to_py_err)
        })
    }

    /// Parent of a node.
    fn parent_of<'py>(&self, py: Python<'py>, node_id: String) -> PyResult<Bound<'py, PyAny>> {
        let nav = Arc::clone(&self.inner);
        future_into_py(py, async move {
            let nav = nav.lock().await;
            nav.parent_of(&node_id)
                .await
                .map(|opt| opt.map(id_to_str))
                .map_err(to_py_err)
        })
    }

    /// Depth of a node in the tree.
    fn depth_of<'py>(&self, py: Python<'py>, node_id: String) -> PyResult<Bound<'py, PyAny>> {
        let nav = Arc::clone(&self.inner);
        future_into_py(py, async move {
            let nav = nav.lock().await;
            nav.depth_of(&node_id).await.map_err(to_py_err)
        })
    }

    /// Title of a node.
    fn node_title<'py>(&self, py: Python<'py>, node_id: String) -> PyResult<Bound<'py, PyAny>> {
        let nav = Arc::clone(&self.inner);
        future_into_py(py, async move {
            let nav = nav.lock().await;
            nav.node_title(&node_id).await.map_err(to_py_err)
        })
    }

    /// All node ids in the tree.
    fn all_node_ids<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let nav = Arc::clone(&self.inner);
        future_into_py(py, async move {
            let nav = nav.lock().await;
            Ok(nav
                .all_node_ids()
                .await
                .into_iter()
                .map(id_to_str)
                .collect::<Vec<_>>())
        })
    }

    // ── P1: Extended tools ────────────────────────────────────────────

    /// Go back to the previous position (navigation history).
    fn back<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let nav = Arc::clone(&self.inner);
        future_into_py(py, async move {
            let mut nav = nav.lock().await;
            nav.back().await.map_err(to_py_err)
        })
    }

    /// Return the full table of contents.
    fn toc<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let nav = Arc::clone(&self.inner);
        future_into_py(py, async move {
            let nav = nav.lock().await;
            Ok(nav
                .toc()
                .await
                .into_iter()
                .map(PyTocEntry::from)
                .collect::<Vec<_>>())
        })
    }

    /// Get statistics about a node (or current node if None).
    #[pyo3(signature = (node_id=None))]
    fn stats<'py>(&self, py: Python<'py>, node_id: Option<String>) -> PyResult<Bound<'py, PyAny>> {
        let nav = Arc::clone(&self.inner);
        future_into_py(py, async move {
            let nav = nav.lock().await;
            nav.stats(node_id.as_deref())
                .await
                .map(PyNodeStats::from)
                .map_err(to_py_err)
        })
    }

    /// Search within a specific node's content (no cursor movement).
    fn grep_node<'py>(
        &self,
        py: Python<'py>,
        node_id: String,
        pattern: String,
    ) -> PyResult<Bound<'py, PyAny>> {
        let nav = Arc::clone(&self.inner);
        future_into_py(py, async move {
            let nav = nav.lock().await;
            nav.grep_node(&node_id, &pattern)
                .await
                .map(|r| r.into_iter().map(PyMatchResult::from).collect::<Vec<_>>())
                .map_err(to_py_err)
        })
    }

    /// Find semantically similar nodes.
    fn similar<'py>(&self, py: Python<'py>, node_id: String) -> PyResult<Bound<'py, PyAny>> {
        let nav = Arc::clone(&self.inner);
        future_into_py(py, async move {
            let nav = nav.lock().await;
            Ok(nav
                .similar(&node_id)
                .await
                .into_iter()
                .map(PySimilarResult::from)
                .collect::<Vec<_>>())
        })
    }

    /// Get the pre-computed overview for a section.
    fn section_overview<'py>(
        &self,
        py: Python<'py>,
        node_id: String,
    ) -> PyResult<Bound<'py, PyAny>> {
        let nav = Arc::clone(&self.inner);
        future_into_py(py, async move {
            let nav = nav.lock().await;
            nav.section_overview(&node_id).await.map_err(to_py_err)
        })
    }

    /// List sibling nodes at the same level as a given node.
    fn siblings<'py>(
        &self,
        py: Python<'py>,
        node_id: Option<String>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let nav = Arc::clone(&self.inner);
        future_into_py(py, async move {
            let nav = nav.lock().await;
            nav.siblings(node_id.as_deref())
                .await
                .map(|v| v.into_iter().map(PyNodeInfo::from).collect::<Vec<_>>())
                .map_err(to_py_err)
        })
    }

    /// List ancestors from root to a given node, inclusive.
    fn ancestors<'py>(
        &self,
        py: Python<'py>,
        node_id: Option<String>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let nav = Arc::clone(&self.inner);
        future_into_py(py, async move {
            let nav = nav.lock().await;
            nav.ancestors(node_id.as_deref())
                .await
                .map(|v| v.into_iter().map(PyNodeInfo::from).collect::<Vec<_>>())
                .map_err(to_py_err)
        })
    }

    /// Document-level overview card.
    fn doc_card<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let nav = Arc::clone(&self.inner);
        future_into_py(py, async move {
            let nav = nav.lock().await;
            Ok(nav.doc_card().await.map(PyDocCard::from))
        })
    }

    /// Key concepts extracted from the document.
    fn concepts<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let nav = Arc::clone(&self.inner);
        future_into_py(py, async move {
            let nav = nav.lock().await;
            Ok(nav
                .concepts()
                .await
                .into_iter()
                .map(PyConceptInfo::from)
                .collect::<Vec<_>>())
        })
    }

    /// Find a section by exact title (case-insensitive).
    fn find_section<'py>(&self, py: Python<'py>, title: String) -> PyResult<Bound<'py, PyAny>> {
        let nav = Arc::clone(&self.inner);
        future_into_py(py, async move {
            let nav = nav.lock().await;
            Ok(nav.find_section(&title).await.map(PyFindResult::from))
        })
    }
}

// =========================================================================
// Helper types
// =========================================================================

/// Information about a node in the document tree.
#[pyclass(name = "NodeInfo", skip_from_py_object)]
#[derive(Clone)]
pub struct PyNodeInfo {
    #[pyo3(get)]
    pub id: String,
    #[pyo3(get)]
    pub title: String,
    #[pyo3(get)]
    pub depth: usize,
    #[pyo3(get)]
    pub child_count: usize,
    #[pyo3(get)]
    pub leaf_count: usize,
    #[pyo3(get)]
    pub question_hints: Vec<String>,
    #[pyo3(get)]
    pub topic_tags: Vec<String>,
}

impl From<NodeInfo> for PyNodeInfo {
    fn from(v: NodeInfo) -> Self {
        Self {
            id: id_to_str(v.id),
            title: v.title,
            depth: v.depth,
            child_count: v.child_count,
            leaf_count: v.leaf_count,
            question_hints: v.question_hints,
            topic_tags: v.topic_tags,
        }
    }
}

/// A regex match within node content.
#[pyclass(name = "MatchResult", skip_from_py_object)]
#[derive(Clone)]
pub struct PyMatchResult {
    #[pyo3(get)]
    pub node_id: String,
    #[pyo3(get)]
    pub title: String,
    #[pyo3(get)]
    pub snippet: String,
    #[pyo3(get)]
    pub line_number: usize,
}

impl From<MatchResult> for PyMatchResult {
    fn from(v: MatchResult) -> Self {
        Self {
            node_id: id_to_str(v.node_id),
            title: v.title,
            snippet: v.snippet,
            line_number: v.line_number,
        }
    }
}

/// A node found by search.
#[pyclass(name = "FindResult", skip_from_py_object)]
#[derive(Clone)]
pub struct PyFindResult {
    #[pyo3(get)]
    pub node_id: String,
    #[pyo3(get)]
    pub title: String,
    #[pyo3(get)]
    pub depth: usize,
    #[pyo3(get)]
    pub leaf_count: usize,
}

impl From<FindResult> for PyFindResult {
    fn from(v: FindResult) -> Self {
        Self {
            node_id: id_to_str(v.node_id),
            title: v.title,
            depth: v.depth,
            leaf_count: v.leaf_count,
        }
    }
}

/// Word/line/character count.
#[pyclass(name = "WordCount", skip_from_py_object)]
#[derive(Clone)]
pub struct PyWordCount {
    #[pyo3(get)]
    pub lines: usize,
    #[pyo3(get)]
    pub words: usize,
    #[pyo3(get)]
    pub chars: usize,
}

impl From<WordCount> for PyWordCount {
    fn from(v: WordCount) -> Self {
        Self {
            lines: v.lines,
            words: v.words,
            chars: v.chars,
        }
    }
}

/// Evidence collected during navigation.
#[pyclass(name = "CollectedEvidence", skip_from_py_object)]
#[derive(Clone)]
pub struct PyCollectedEvidence {
    #[pyo3(get)]
    pub node_id: String,
    #[pyo3(get)]
    pub title: String,
    #[pyo3(get)]
    pub content: String,
    #[pyo3(get)]
    pub source_path: String,
}

impl From<CollectedEvidence> for PyCollectedEvidence {
    fn from(v: CollectedEvidence) -> Self {
        Self {
            node_id: id_to_str(v.node_id),
            title: v.title,
            content: v.content,
            source_path: v.source_path,
        }
    }
}

/// A topic entry from the reasoning index.
#[pyclass(name = "TopicEntry", skip_from_py_object)]
#[derive(Clone)]
pub struct PyTopicEntry {
    #[pyo3(get)]
    pub node_id: String,
    #[pyo3(get)]
    pub weight: f32,
    #[pyo3(get)]
    pub depth: usize,
}

impl From<TopicEntryInfo> for PyTopicEntry {
    fn from(v: TopicEntryInfo) -> Self {
        Self {
            node_id: id_to_str(v.node_id),
            weight: v.weight,
            depth: v.depth,
        }
    }
}

/// A section summary from the reasoning index.
#[pyclass(name = "SectionSummary", skip_from_py_object)]
#[derive(Clone)]
pub struct PySectionSummary {
    #[pyo3(get)]
    pub node_id: String,
    #[pyo3(get)]
    pub title: String,
    #[pyo3(get)]
    pub summary: String,
    #[pyo3(get)]
    pub depth: usize,
}

impl From<SectionSummaryInfo> for PySectionSummary {
    fn from(v: SectionSummaryInfo) -> Self {
        Self {
            node_id: id_to_str(v.node_id),
            title: v.title,
            summary: v.summary,
            depth: v.depth,
        }
    }
}

// =========================================================================
// P1: New helper types
// =========================================================================

/// A single entry in the table of contents.
#[pyclass(name = "TocEntry", skip_from_py_object)]
#[derive(Clone)]
pub struct PyTocEntry {
    #[pyo3(get)]
    pub id: String,
    #[pyo3(get)]
    pub title: String,
    #[pyo3(get)]
    pub depth: usize,
    #[pyo3(get)]
    pub child_count: usize,
}

impl From<TocEntry> for PyTocEntry {
    fn from(v: TocEntry) -> Self {
        Self {
            id: id_to_str(v.id),
            title: v.title,
            depth: v.depth,
            child_count: v.child_count,
        }
    }
}

/// Statistics about a node.
#[pyclass(name = "NodeStats", skip_from_py_object)]
#[derive(Clone)]
pub struct PyNodeStats {
    #[pyo3(get)]
    pub id: String,
    #[pyo3(get)]
    pub title: String,
    #[pyo3(get)]
    pub depth: usize,
    #[pyo3(get)]
    pub child_count: usize,
    #[pyo3(get)]
    pub leaf_count: usize,
    #[pyo3(get)]
    pub char_count: usize,
    #[pyo3(get)]
    pub word_count: usize,
    #[pyo3(get)]
    pub is_leaf: bool,
}

impl From<NodeStats> for PyNodeStats {
    fn from(v: NodeStats) -> Self {
        Self {
            id: id_to_str(v.id),
            title: v.title,
            depth: v.depth,
            child_count: v.child_count,
            leaf_count: v.leaf_count,
            char_count: v.char_count,
            word_count: v.word_count,
            is_leaf: v.is_leaf,
        }
    }
}

/// A node found by semantic similarity.
#[pyclass(name = "SimilarResult", skip_from_py_object)]
#[derive(Clone)]
pub struct PySimilarResult {
    #[pyo3(get)]
    pub id: String,
    #[pyo3(get)]
    pub title: String,
    #[pyo3(get)]
    pub relevance: f32,
    #[pyo3(get)]
    pub shared_keywords: Vec<String>,
}

impl From<SimilarResult> for PySimilarResult {
    fn from(v: SimilarResult) -> Self {
        Self {
            id: id_to_str(v.id),
            title: v.title,
            relevance: v.relevance,
            shared_keywords: v.shared_keywords,
        }
    }
}

/// A top-level section in a document card.
#[pyclass(name = "SectionCard", skip_from_py_object)]
#[derive(Clone)]
pub struct PySectionCard {
    #[pyo3(get)]
    pub title: String,
    #[pyo3(get)]
    pub description: String,
    #[pyo3(get)]
    pub leaf_count: usize,
}

impl From<SectionCardInfo> for PySectionCard {
    fn from(v: SectionCardInfo) -> Self {
        Self {
            title: v.title,
            description: v.description,
            leaf_count: v.leaf_count,
        }
    }
}

/// Document-level overview card.
#[pyclass(name = "DocCard", skip_from_py_object)]
#[derive(Clone)]
pub struct PyDocCard {
    #[pyo3(get)]
    pub title: String,
    #[pyo3(get)]
    pub overview: String,
    #[pyo3(get)]
    pub question_hints: Vec<String>,
    #[pyo3(get)]
    pub topic_tags: Vec<String>,
    #[pyo3(get)]
    pub sections: Vec<PySectionCard>,
    #[pyo3(get)]
    pub total_leaves: usize,
}

impl From<DocCardInfo> for PyDocCard {
    fn from(v: DocCardInfo) -> Self {
        Self {
            title: v.title,
            overview: v.overview,
            question_hints: v.question_hints,
            topic_tags: v.topic_tags,
            sections: v.sections.into_iter().map(PySectionCard::from).collect(),
            total_leaves: v.total_leaves,
        }
    }
}

/// A key concept extracted from the document.
#[pyclass(name = "ConceptInfo", skip_from_py_object)]
#[derive(Clone)]
pub struct PyConceptInfo {
    #[pyo3(get)]
    pub name: String,
    #[pyo3(get)]
    pub summary: String,
    #[pyo3(get)]
    pub sections: Vec<String>,
}

impl From<ConceptInfo> for PyConceptInfo {
    fn from(v: ConceptInfo) -> Self {
        Self {
            name: v.name,
            summary: v.summary,
            sections: v.sections,
        }
    }
}
