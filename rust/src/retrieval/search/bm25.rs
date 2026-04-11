// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! BM25 scoring module using the `bm25` crate.
//!
//! This module provides:
//! - Per-field weighting for document scoring
//! - Configurable length normalization
//! - IDF caching for efficient scoring
//! - Query expansion support

use bm25::{
    Embedder, EmbedderBuilder, Language, Scorer, ScoredDocument,
    DefaultTokenizer, Tokenizer,
};

/// Field weights for BM25 scoring.
///
/// Different document fields can have different importance.
/// For example, title matches are typically more important than content matches.
#[derive(Debug, Clone, Copy)]
pub struct FieldWeights {
    /// Weight for title field matches.
    pub title: f32,
    /// Weight for summary field matches.
    pub summary: f32,
    /// Weight for content field matches.
    pub content: f32,
}

impl Default for FieldWeights {
    fn default() -> Self {
        Self {
            title: 2.0,
            summary: 1.5,
            content: 1.0,
        }
    }
}

/// BM25 parameters for fine-tuning.
#[derive(Debug, Clone, Copy)]
pub struct Bm25Params {
    /// Term frequency saturation parameter (k1).
    /// Controls how quickly term frequency saturates.
    /// Typical value: 1.2
    pub k1: f32,
    /// Length normalization parameter (b).
    /// Controls how much document length affects scoring.
    /// - 0.0: No length normalization
    /// - 1.0: Full length normalization
    /// Typical value: 0.75
    pub b: f32,
    /// Average document length.
    /// If not known, can be estimated or set to 1.0 with b=0.
    pub avgdl: f32,
}

impl Default for Bm25Params {
    fn default() -> Self {
        Self {
            k1: 1.2,
            b: 0.75,
            avgdl: 100.0,
        }
    }
}

/// A document with multiple fields for scoring.
#[derive(Debug, Clone)]
pub struct FieldDocument<K> {
    /// Document identifier.
    pub id: K,
    /// Title field.
    pub title: String,
    /// Summary field.
    pub summary: String,
    /// Content field.
    pub content: String,
}

impl<K> FieldDocument<K> {
    /// Create a new field document.
    pub fn new(id: K, title: String, summary: String, content: String) -> Self {
        Self { id, title, summary, content }
    }

    /// Get combined text for embedding.
    fn combined_text(&self) -> String {
        format!("{} {} {}", self.title, self.summary, self.content)
    }
}

/// Key for field-specific document storage.
#[derive(Debug, Clone, Hash, Eq, PartialEq)]
struct FieldKey<K> {
    doc_id: K,
    field: Field,
}

#[derive(Debug, Clone, Copy, Hash, Eq, PartialEq)]
enum Field {
    Title,
    Summary,
    Content,
}

/// BM25 engine with per-field weighting support.
///
/// This wraps the `bm25` crate's Embedder and Scorer to provide:
/// - Per-field weighting
/// - Configurable parameters
/// - IDF caching (handled internally by Scorer)
pub struct Bm25Engine<K> {
    /// The embedder for creating sparse vectors.
    embedder: Embedder,
    /// The scorer for scoring documents (combined text).
    scorer: Scorer<K>,
    /// Field-specific scorers for weighted scoring.
    title_scorer: Scorer<K>,
    summary_scorer: Scorer<K>,
    content_scorer: Scorer<K>,
    /// Field weights.
    weights: FieldWeights,
    /// Document count.
    doc_count: usize,
    /// Whether the engine has been fitted to a corpus.
    fitted: bool,
}

impl<K: std::hash::Hash + Eq + Clone + std::fmt::Debug> Bm25Engine<K> {
    /// Create a new BM25 engine with default parameters.
    pub fn new() -> Self {
        Self::with_params(Bm25Params::default())
    }

    /// Create a BM25 engine with custom parameters.
    pub fn with_params(params: Bm25Params) -> Self {
        let embedder = EmbedderBuilder::with_avgdl(params.avgdl)
            .k1(params.k1)
            .b(params.b)
            .language_mode(Language::English)
            .build();

        Self {
            embedder,
            scorer: Scorer::new(),
            title_scorer: Scorer::new(),
            summary_scorer: Scorer::new(),
            content_scorer: Scorer::new(),
            weights: FieldWeights::default(),
            doc_count: 0,
            fitted: false,
        }
    }

    /// Create a BM25 engine fitted to a corpus.
    ///
    /// This calculates the true average document length from the corpus.
    pub fn fit_to_corpus(documents: &[FieldDocument<K>]) -> Self {
        // Collect owned strings first
        let corpus: Vec<String> = documents.iter()
            .map(|d| d.combined_text())
            .collect();
        let corpus_refs: Vec<&str> = corpus.iter().map(|s| s.as_str()).collect();

        let embedder = EmbedderBuilder::with_fit_to_corpus(Language::English, &corpus_refs)
            .build();

        let mut engine = Self {
            embedder,
            scorer: Scorer::new(),
            title_scorer: Scorer::new(),
            summary_scorer: Scorer::new(),
            content_scorer: Scorer::new(),
            weights: FieldWeights::default(),
            doc_count: 0,
            fitted: true,
        };

        // Index all documents
        for doc in documents {
            engine.upsert(doc);
        }

        engine
    }

    /// Set field weights.
    pub fn with_weights(mut self, weights: FieldWeights) -> Self {
        self.weights = weights;
        self
    }

    /// Set language for tokenization.
    pub fn with_language(mut self, language: Language) -> Self {
        self.embedder = EmbedderBuilder::with_avgdl(self.embedder.avgdl())
            .language_mode(language)
            .build();
        self
    }

    /// Get the average document length.
    pub fn avgdl(&self) -> f32 {
        self.embedder.avgdl()
    }

    /// Check if the engine has been fitted to a corpus.
    pub fn is_fitted(&self) -> bool {
        self.fitted
    }

    /// Upsert a document into the index.
    ///
    /// This stores embeddings for each field separately for weighted scoring.
    pub fn upsert(&mut self, document: &FieldDocument<K>) {
        let id = &document.id;

        // Embed and store each field separately
        let title_emb = self.embedder.embed(&document.title);
        let summary_emb = self.embedder.embed(&document.summary);
        let content_emb = self.embedder.embed(&document.content);

        self.title_scorer.upsert(id, title_emb);
        self.summary_scorer.upsert(id, summary_emb);
        self.content_scorer.upsert(id, content_emb);

        // Also store combined embedding for basic search
        let combined = self.embedder.embed(&document.combined_text());
        self.scorer.upsert(id, combined);

        self.doc_count += 1;
    }

    /// Remove a document from the index.
    pub fn remove(&mut self, id: &K) {
        self.scorer.remove(id);
        self.title_scorer.remove(id);
        self.summary_scorer.remove(id);
        self.content_scorer.remove(id);
        self.doc_count = self.doc_count.saturating_sub(1);
    }

    /// Get the number of indexed documents.
    pub fn len(&self) -> usize {
        self.doc_count
    }

    /// Check if the index is empty.
    pub fn is_empty(&self) -> bool {
        self.doc_count == 0
    }

    /// Score a single document against a query.
    ///
    /// Returns None if the document is not in the index.
    pub fn score(&self, id: &K, query: &str) -> Option<f32> {
        let query_emb = self.embedder.embed(query);

        // Score each field
        let title_score = self.title_scorer.score(id, &query_emb)?;
        let summary_score = self.summary_scorer.score(id, &query_emb)?;
        let content_score = self.content_scorer.score(id, &query_emb)?;

        // Weighted combination
        let total_weight = self.weights.title + self.weights.summary + self.weights.content;
        let weighted_score = (title_score * self.weights.title
            + summary_score * self.weights.summary
            + content_score * self.weights.content) / total_weight;

        Some(weighted_score)
    }

    /// Search for documents matching a query.
    ///
    /// Returns documents sorted by score (descending).
    pub fn search(&self, query: &str, limit: usize) -> Vec<ScoredDocument<K>> {
        let query_emb = self.embedder.embed(query);
        self.scorer.matches(&query_emb).into_iter().take(limit).collect()
    }

    /// Search with per-field weighting.
    ///
    /// This is slower but provides more accurate weighted scores.
    pub fn search_weighted(&self, query: &str, limit: usize) -> Vec<(K, f32)> {
        let query_emb = self.embedder.embed(query);

        // Get all document IDs from the main scorer
        let all_results = self.scorer.matches(&query_emb);

        let mut scored: Vec<(K, f32)> = all_results
            .into_iter()
            .filter_map(|scored_doc| {
                let id = scored_doc.id;

                // Get per-field scores
                let title_score = self.title_scorer.score(&id, &query_emb)?;
                let summary_score = self.summary_scorer.score(&id, &query_emb)?;
                let content_score = self.content_scorer.score(&id, &query_emb)?;

                let total_weight = self.weights.title + self.weights.summary + self.weights.content;
                let weighted_score = (title_score * self.weights.title
                    + summary_score * self.weights.summary
                    + content_score * self.weights.content) / total_weight;

                Some((id, weighted_score))
            })
            .collect();

        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(limit);
        scored
    }

    /// Extract keywords from a query (tokenize and filter).
    pub fn tokenize(&self, text: &str) -> Vec<String> {
        let tokenizer = DefaultTokenizer::builder()
            .language_mode(Language::English)
            .normalization(true)
            .stopwords(true)
            .stemming(true)
            .build();
        tokenizer.tokenize(text)
    }

    /// Get the underlying embedder.
    pub fn embedder(&self) -> &Embedder {
        &self.embedder
    }

    /// Get mutable access to the embedder.
    pub fn embedder_mut(&mut self) -> &mut Embedder {
        &mut self.embedder
    }
}

impl<K: std::hash::Hash + Eq + Clone + std::fmt::Debug> Default for Bm25Engine<K> {
    fn default() -> Self {
        Self::new()
    }
}

/// Query expansion result from LLM.
#[derive(Debug, Clone)]
pub struct ExpandedQuery {
    /// Original query.
    pub original: String,
    /// Expanded terms.
    pub expansions: Vec<String>,
    /// Combined query (original + expansions).
    pub combined: String,
}

impl ExpandedQuery {
    /// Create a new expanded query.
    pub fn new(original: String, expansions: Vec<String>) -> Self {
        let combined = format!("{} {}", original, expansions.join(" "));
        Self { original, expansions, combined }
    }
}

/// Query expander trait for LLM-based expansion.
#[async_trait::async_trait]
pub trait QueryExpander: Send + Sync {
    /// Expand a query with related terms.
    async fn expand(&self, query: &str) -> ExpandedQuery;
}

/// Common English stop words for keyword filtering.
pub const STOPWORDS: &[&str] = &[
    "a", "an", "the", "is", "are", "was", "were", "be", "been", "being",
    "have", "has", "had", "do", "does", "did", "will", "would", "could",
    "should", "may", "might", "must", "shall", "can", "need", "dare",
    "ought", "used", "to", "of", "in", "for", "on", "with", "at", "by",
    "from", "as", "into", "through", "during", "before", "after", "above",
    "below", "between", "under", "again", "further", "then", "once",
    "here", "there", "when", "where", "why", "how", "all", "each", "few",
    "more", "most", "other", "some", "such", "no", "nor", "not", "only",
    "own", "same", "so", "than", "too", "very", "just", "and", "but",
    "if", "or", "because", "until", "while", "about", "what", "which",
    "who", "whom", "this", "that", "these", "those", "i", "me", "my",
    "myself", "we", "our", "ours", "ourselves", "you", "your", "yours",
    "yourself", "yourselves", "he", "him", "his", "himself", "she", "her",
    "hers", "herself", "it", "its", "itself", "they", "them", "their",
    "theirs", "themselves",
];

/// Extract keywords from a query string, filtering stop words.
///
/// This is a simple keyword extraction that:
/// - Converts to lowercase
/// - Splits on non-alphanumeric characters
/// - Filters out stop words
/// - Requires minimum length of 2 characters
#[must_use]
pub fn extract_keywords(query: &str) -> Vec<String> {
    query
        .to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|s| {
            let s = *s;
            !s.is_empty() && s.len() > 1 && !STOPWORDS.contains(&s)
        })
        .map(String::from)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bm25_engine_creation() {
        let engine: Bm25Engine<u32> = Bm25Engine::new();
        assert!(engine.is_empty());
        assert!(!engine.is_fitted());
    }

    #[test]
    fn test_bm25_engine_fit_to_corpus() {
        let docs = vec![
            FieldDocument::new(1u32, "Rust Programming".to_string(), "About Rust".to_string(), "Rust is a systems programming language.".to_string()),
            FieldDocument::new(2u32, "Python Guide".to_string(), "About Python".to_string(), "Python is a scripting language.".to_string()),
        ];

        let engine = Bm25Engine::fit_to_corpus(&docs);
        assert!(engine.is_fitted());
        assert_eq!(engine.len(), 2);
    }

    #[test]
    fn test_bm25_search() {
        let docs = vec![
            FieldDocument::new(1u32, "Rust Programming".to_string(), "About Rust".to_string(), "Rust is a systems programming language with memory safety.".to_string()),
            FieldDocument::new(2u32, "Python Guide".to_string(), "About Python".to_string(), "Python is a scripting language for data science.".to_string()),
            FieldDocument::new(3u32, "Rust Memory Safety".to_string(), "Memory in Rust".to_string(), "Rust provides guaranteed memory safety without garbage collection.".to_string()),
        ];

        let engine = Bm25Engine::fit_to_corpus(&docs);
        let results = engine.search("rust memory", 10);

        assert!(!results.is_empty());
        // Documents about Rust should rank higher
        assert!(results.iter().any(|r| r.id == 1 || r.id == 3));
    }

    #[test]
    fn test_bm25_weighted_search() {
        let docs = vec![
            FieldDocument::new(1u32, "Rust Programming".to_string(), "About memory safety".to_string(), "Content about other things.".to_string()),
            FieldDocument::new(2u32, "Other Language".to_string(), "About other things".to_string(), "Rust memory safety is important.".to_string()),
        ];

        let engine = Bm25Engine::fit_to_corpus(&docs)
            .with_weights(FieldWeights {
                title: 3.0,
                summary: 2.0,
                content: 1.0,
            });

        let results = engine.search_weighted("rust", 10);

        // Doc 1 has "Rust" in title, should rank higher
        assert_eq!(results.first().map(|(id, _)| *id), Some(1u32));
    }

    #[test]
    fn test_bm25_score() {
        let docs = vec![
            FieldDocument::new(1u32, "Rust Programming".to_string(), "About Rust".to_string(), "Rust is a systems programming language.".to_string()),
        ];

        let engine = Bm25Engine::fit_to_corpus(&docs);
        let score = engine.score(&1u32, "rust programming");

        assert!(score.is_some());
        assert!(score.unwrap() > 0.0);
    }

    #[test]
    fn test_bm25_tokenize() {
        let engine: Bm25Engine<u32> = Bm25Engine::new();
        let tokens = engine.tokenize("What is the Rust programming language?");

        // Should filter stop words and stem
        assert!(tokens.contains(&"rust".to_string()));
        assert!(tokens.contains(&"program".to_string())); // stemmed
        assert!(!tokens.contains(&"what".to_string())); // stop word
        assert!(!tokens.contains(&"the".to_string())); // stop word
    }

    #[test]
    fn test_bm25_remove() {
        let docs = vec![
            FieldDocument::new(1u32, "Rust".to_string(), "About Rust".to_string(), "Rust content.".to_string()),
        ];

        let mut engine = Bm25Engine::fit_to_corpus(&docs);
        assert_eq!(engine.len(), 1);

        engine.remove(&1u32);
        assert!(engine.is_empty());
    }

    #[test]
    fn test_field_weights_default() {
        let weights = FieldWeights::default();
        assert!((weights.title - 2.0).abs() < f32::EPSILON);
        assert!((weights.summary - 1.5).abs() < f32::EPSILON);
        assert!((weights.content - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_bm25_params_default() {
        let params = Bm25Params::default();
        assert!((params.k1 - 1.2).abs() < f32::EPSILON);
        assert!((params.b - 0.75).abs() < f32::EPSILON);
        assert!((params.avgdl - 100.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_expanded_query() {
        let expanded = ExpandedQuery::new(
            "rust".to_string(),
            vec!["programming".to_string(), "language".to_string()],
        );

        assert_eq!(expanded.original, "rust");
        assert_eq!(expanded.expansions.len(), 2);
        assert_eq!(expanded.combined, "rust programming language");
    }
}
