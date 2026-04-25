// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Keyword extraction from query strings.

/// Common English stop words for keyword filtering.
pub const STOPWORDS: &[&str] = &[
    "a", "an", "the", "is", "are", "was", "were", "be", "been", "being", "have", "has", "had",
    "do", "does", "did", "will", "would", "could", "should", "may", "might", "must", "shall",
    "can", "need", "dare", "ought", "used", "to", "of", "in", "for", "on", "with", "at", "by",
    "from", "as", "into", "through", "during", "before", "after", "above", "below", "between",
    "under", "again", "further", "then", "once", "here", "there", "when", "where", "why", "how",
    "all", "each", "few", "more", "most", "other", "some", "such", "no", "nor", "not", "only",
    "own", "same", "so", "than", "too", "very", "just", "and", "but", "if", "or", "because",
    "until", "while", "about", "what", "which", "who", "whom", "this", "that", "these", "those",
    "i", "me", "my", "myself", "we", "our", "ours", "ourselves", "you", "your", "yours",
    "yourself", "yourselves", "he", "him", "his", "himself", "she", "her", "hers", "herself",
    "it", "its", "itself", "they", "them", "their", "theirs", "themselves",
];

/// Extract keywords from a query string, filtering stop words.
///
/// Converts to lowercase, splits on non-alphanumeric characters,
/// filters out stop words, and requires minimum length of 2 characters.
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
    fn test_extract_keywords_basic() {
        let keywords = extract_keywords("What is the Rust programming language?");
        assert!(keywords.contains(&"rust".to_string()));
        assert!(keywords.contains(&"programming".to_string()));
        assert!(keywords.contains(&"language".to_string()));
        assert!(!keywords.contains(&"what".to_string()));
        assert!(!keywords.contains(&"is".to_string()));
        assert!(!keywords.contains(&"the".to_string()));
    }

    #[test]
    fn test_extract_keywords_empty() {
        assert!(extract_keywords("").is_empty());
        assert!(extract_keywords("is the a").is_empty());
    }

    #[test]
    fn test_extract_keywords_single_char() {
        let keywords = extract_keywords("a b c rust");
        assert_eq!(keywords, vec!["rust".to_string()]);
    }
}
