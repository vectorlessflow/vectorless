// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Configuration documentation generation.
//!
//! This module provides utilities for generating documentation
//! from configuration types, including markdown reference and
//! example TOML files.

use super::types::Config;

/// Configuration documentation generator.
#[derive(Debug, Clone)]
pub struct ConfigDocs {
    config: Config,
}

impl ConfigDocs {
    /// Create a new documentation generator.
    pub fn new(config: Config) -> Self {
        Self { config }
    }

    /// Create with default configuration.
    pub fn with_defaults() -> Self {
        Self::new(Config::default())
    }

    /// Generate markdown documentation for the configuration.
    pub fn to_markdown(&self) -> String {
        let mut md = String::new();

        md.push_str("# Configuration Reference\n\n");
        md.push_str("This document describes all configuration options for vectorless.\n\n");
        md.push_str("## Configuration File\n\n");
        md.push_str("Configuration is loaded from a TOML file. Default locations:\n");
        md.push_str("- `./vectorless.toml`\n");
        md.push_str("- `./config.toml`\n");
        md.push_str("- `./.vectorless.toml`\n\n");

        // Indexer section
        md.push_str("## `[indexer]`\n\n");
        md.push_str("Controls document indexing behavior.\n\n");
        md.push_str("| Option | Type | Default | Description |\n");
        md.push_str("|--------|------|---------|-------------|\n");
        self.add_row(&mut md, "subsection_threshold", "usize", "300",
            "Word count threshold for splitting sections into subsections");
        self.add_row(&mut md, "max_segment_tokens", "usize", "3000",
            "Maximum tokens to send in a single segmentation request");
        self.add_row(&mut md, "max_summary_tokens", "usize", "200",
            "Maximum tokens for each summary");
        self.add_row(&mut md, "min_summary_tokens", "usize", "20",
            "Minimum content tokens required to generate a summary");
        md.push_str("\n");

        // Summary section
        md.push_str("## `[summary]`\n\n");
        md.push_str("LLM configuration for summary generation.\n\n");
        md.push_str("| Option | Type | Default | Description |\n");
        md.push_str("|--------|------|---------|-------------|\n");
        self.add_row(&mut md, "model", "string", "gpt-4o-mini", "Model for summarization");
        self.add_row(&mut md, "endpoint", "string", "https://api.openai.com/v1", "API endpoint");
        self.add_row(&mut md, "api_key", "string?", "null", "API key (optional, can use env var)");
        self.add_row(&mut md, "max_tokens", "usize", "200", "Maximum tokens for summary generation");
        self.add_row(&mut md, "temperature", "f32", "0.0", "Temperature for summary generation");
        md.push_str("\n");

        // Retrieval section
        md.push_str("## `[retrieval]`\n\n");
        md.push_str("Retrieval model and behavior configuration.\n\n");
        md.push_str("| Option | Type | Default | Description |\n");
        md.push_str("|--------|------|---------|-------------|\n");
        self.add_row(&mut md, "model", "string", "gpt-4o", "Model for retrieval navigation");
        self.add_row(&mut md, "endpoint", "string", "https://api.openai.com/v1", "API endpoint");
        self.add_row(&mut md, "api_key", "string?", "null", "API key (defaults to summary.api_key)");
        self.add_row(&mut md, "top_k", "usize", "3", "Number of top results to return");
        self.add_row(&mut md, "max_tokens", "usize", "1000", "Maximum tokens for retrieval context");
        self.add_row(&mut md, "temperature", "f32", "0.0", "Temperature for retrieval");
        md.push_str("\n");

        // Retrieval.search section
        md.push_str("## `[retrieval.search]`\n\n");
        md.push_str("Search algorithm configuration.\n\n");
        md.push_str("| Option | Type | Default | Description |\n");
        md.push_str("|--------|------|---------|-------------|\n");
        self.add_row(&mut md, "top_k", "usize", "5", "Number of top-k results to return");
        self.add_row(&mut md, "beam_width", "usize", "3", "Beam width for multi-path search");
        self.add_row(&mut md, "max_iterations", "usize", "10", "Maximum iterations for search algorithms");
        self.add_row(&mut md, "min_score", "f32", "0.1", "Minimum score to include a path");
        md.push_str("\n");

        // Retrieval.sufficiency section
        md.push_str("## `[retrieval.sufficiency]`\n\n");
        md.push_str("Sufficiency checker configuration.\n\n");
        md.push_str("| Option | Type | Default | Description |\n");
        md.push_str("|--------|------|---------|-------------|\n");
        self.add_row(&mut md, "min_tokens", "usize", "500", "Minimum tokens for sufficiency");
        self.add_row(&mut md, "target_tokens", "usize", "2000", "Target tokens for full sufficiency");
        self.add_row(&mut md, "max_tokens", "usize", "4000", "Maximum tokens before stopping");
        self.add_row(&mut md, "min_content_length", "usize", "200", "Minimum content length (characters)");
        self.add_row(&mut md, "confidence_threshold", "f32", "0.7", "Confidence threshold for LLM judge");
        md.push_str("\n");

        // Retrieval.content section
        md.push_str("## `[retrieval.content]`\n\n");
        md.push_str("Content aggregator configuration.\n\n");
        md.push_str("| Option | Type | Default | Description |\n");
        md.push_str("|--------|------|---------|-------------|\n");
        self.add_row(&mut md, "enabled", "bool", "true", "Enable content aggregator");
        self.add_row(&mut md, "token_budget", "usize", "4000", "Maximum tokens for aggregated content");
        self.add_row(&mut md, "min_relevance_score", "f32", "0.2", "Minimum relevance score threshold (0.0-1.0)");
        self.add_row(&mut md, "scoring_strategy", "string", "keyword_bm25", "Scoring strategy (keyword_only, keyword_bm25, hybrid)");
        self.add_row(&mut md, "output_format", "string", "markdown", "Output format (markdown, json, tree, flat)");
        self.add_row(&mut md, "include_scores", "bool", "false", "Include relevance scores in output");
        self.add_row(&mut md, "hierarchical_min_per_level", "f32", "0.1", "Minimum budget allocation per depth level");
        self.add_row(&mut md, "deduplicate", "bool", "true", "Enable content deduplication");
        self.add_row(&mut md, "dedup_threshold", "f32", "0.9", "Similarity threshold for deduplication");
        md.push_str("\n");

        // Retrieval.strategy section
        md.push_str("## `[retrieval.strategy]`\n\n");
        md.push_str("Strategy-specific configuration.\n\n");
        md.push_str("| Option | Type | Default | Description |\n");
        md.push_str("|--------|------|---------|-------------|\n");
        self.add_row(&mut md, "exploration_weight", "f32", "1.414", "MCTS exploration weight (√2)");
        self.add_row(&mut md, "similarity_threshold", "f32", "0.5", "Semantic similarity threshold");
        self.add_row(&mut md, "high_similarity_threshold", "f32", "0.8", "High similarity for 'answer' decision");
        self.add_row(&mut md, "low_similarity_threshold", "f32", "0.3", "Low similarity for 'explore' decision");
        md.push_str("\n");

        // Storage section
        md.push_str("## `[storage]`\n\n");
        md.push_str("Storage configuration.\n\n");
        md.push_str("| Option | Type | Default | Description |\n");
        md.push_str("|--------|------|---------|-------------|\n");
        self.add_row(&mut md, "workspace_dir", "string", "./workspace", "Workspace directory for persisted documents");
        md.push_str("\n");

        // Concurrency section
        md.push_str("## `[concurrency]`\n\n");
        md.push_str("Concurrency control configuration.\n\n");
        md.push_str("| Option | Type | Default | Description |\n");
        md.push_str("|--------|------|---------|-------------|\n");
        self.add_row(&mut md, "max_concurrent_requests", "usize", "10", "Maximum concurrent LLM API calls");
        self.add_row(&mut md, "requests_per_minute", "usize", "500", "Rate limit: requests per minute");
        self.add_row(&mut md, "enabled", "bool", "true", "Enable rate limiting");
        self.add_row(&mut md, "semaphore_enabled", "bool", "true", "Enable semaphore-based concurrency");
        md.push_str("\n");

        // Fallback section
        md.push_str("## `[fallback]`\n\n");
        md.push_str("Fallback/error recovery configuration.\n\n");
        md.push_str("| Option | Type | Default | Description |\n");
        md.push_str("|--------|------|---------|-------------|\n");
        self.add_row(&mut md, "enabled", "bool", "true", "Enable graceful degradation");
        self.add_row(&mut md, "models", "[string]", "[\"gpt-4o-mini\", \"glm-4-flash\"]", "Fallback models in priority order");
        self.add_row(&mut md, "endpoints", "[string]", "[]", "Fallback endpoints in priority order");
        self.add_row(&mut md, "on_rate_limit", "string", "retry_then_fallback", "Behavior on rate limit (retry, fallback, retry_then_fallback, fail)");
        self.add_row(&mut md, "on_timeout", "string", "retry_then_fallback", "Behavior on timeout");
        self.add_row(&mut md, "on_all_failed", "string", "return_error", "Behavior when all attempts fail (return_error, return_cache)");
        md.push_str("\n");

        md
    }

    fn add_row(&self, md: &mut String, name: &str, ty: &str, default: &str, desc: &str) {
        md.push_str(&format!("| `{}` | {} | {} | {} |\n", name, ty, default, desc));
    }

    /// Generate an example TOML file with all options.
    pub fn to_example_toml(&self) -> String {
        toml::to_string_pretty(&self.config).unwrap_or_else(|e| {
            format!("# Error generating TOML: {}\n\n# Using default config\n{}",
                e, Self::fallback_toml())
        })
    }

    fn fallback_toml() -> String {
        r#"# Vectorless Configuration Example
# Copy this file to config.toml and fill in your API keys

[indexer]
subsection_threshold = 300
max_segment_tokens = 3000
max_summary_tokens = 200
min_summary_tokens = 20

[summary]
model = "gpt-4o-mini"
endpoint = "https://api.openai.com/v1"
# api_key = "sk-..."
max_tokens = 200
temperature = 0.0

[retrieval]
model = "gpt-4o"
endpoint = "https://api.openai.com/v1"
# api_key = "sk-..."
top_k = 3
max_tokens = 1000
temperature = 0.0

[retrieval.search]
top_k = 5
beam_width = 3
max_iterations = 10
min_score = 0.1

[retrieval.sufficiency]
min_tokens = 500
target_tokens = 2000
max_tokens = 4000
min_content_length = 200
confidence_threshold = 0.7

[retrieval.cache]
max_entries = 1000
ttl_secs = 3600

[retrieval.strategy]
exploration_weight = 1.414
similarity_threshold = 0.5
high_similarity_threshold = 0.8
low_similarity_threshold = 0.3

[retrieval.content]
enabled = true
token_budget = 4000
min_relevance_score = 0.2
scoring_strategy = "keyword_bm25"
output_format = "markdown"
include_scores = false
hierarchical_min_per_level = 0.1
deduplicate = true
dedup_threshold = 0.9

[storage]
workspace_dir = "./workspace"

[concurrency]
max_concurrent_requests = 10
requests_per_minute = 500
enabled = true
semaphore_enabled = true

[fallback]
enabled = true
models = ["gpt-4o-mini", "glm-4-flash"]
on_rate_limit = "retry_then_fallback"
on_timeout = "retry_then_fallback"
on_all_failed = "return_error"
"#.to_string()
    }

    /// Generate a minimal example TOML file.
    pub fn to_minimal_toml(&self) -> String {
        r#"# Minimal Vectorless Configuration
# Most options have sensible defaults

[summary]
api_key = "your-api-key-here"

[retrieval]
top_k = 5
"#.to_string()
    }
}

impl Default for ConfigDocs {
    fn default() -> Self {
        Self::with_defaults()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_docs_markdown() {
        let docs = ConfigDocs::with_defaults();
        let md = docs.to_markdown();

        assert!(md.contains("# Configuration Reference"));
        assert!(md.contains("## `[indexer]`"));
        assert!(md.contains("## `[retrieval]`"));
        assert!(md.contains("## `[retrieval.content]`"));
    }

    #[test]
    fn test_config_docs_toml() {
        let docs = ConfigDocs::with_defaults();
        let toml = docs.to_example_toml();

        assert!(toml.contains("[indexer]"));
        assert!(toml.contains("[retrieval]"));
    }

    #[test]
    fn test_config_docs_minimal_toml() {
        let docs = ConfigDocs::with_defaults();
        let toml = docs.to_minimal_toml();

        assert!(toml.contains("[summary]"));
        assert!(toml.len() < 200); // Should be minimal
    }
}
