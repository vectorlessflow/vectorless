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

        // LLM section (unified)
        md.push_str("## `[llm]`\n\n");
        md.push_str("Unified LLM configuration for all LLM operations.\n\n");
        md.push_str("| Option | Type | Default | Description |\n");
        md.push_str("|--------|------|---------|-------------|\n");
        self.add_row(
            &mut md,
            "api_key",
            "string?",
            "null",
            "Default API key (used by all clients unless overridden)",
        );
        md.push_str("\n");

        // LLM.summary section
        md.push_str("## `[llm.summary]`\n\n");
        md.push_str("Summary client - generates document summaries during indexing.\n\n");
        md.push_str("| Option | Type | Default | Description |\n");
        md.push_str("|--------|------|---------|-------------|\n");
        self.add_row(
            &mut md,
            "model",
            "string",
            "gpt-4o-mini",
            "Model for summarization (fast, cheap model recommended)",
        );
        self.add_row(
            &mut md,
            "endpoint",
            "string",
            "https://api.openai.com/v1",
            "API endpoint",
        );
        self.add_row(
            &mut md,
            "api_key",
            "string?",
            "null",
            "API key (optional, uses default if not set)",
        );
        self.add_row(&mut md, "max_tokens", "usize", "200", "Maximum tokens for summary");
        self.add_row(&mut md, "temperature", "f32", "0.0", "Temperature for generation");
        md.push_str("\n");

        // LLM.retrieval section
        md.push_str("## `[llm.retrieval]`\n\n");
        md.push_str("Retrieval client - used for retrieval decisions and content evaluation.\n\n");
        md.push_str("| Option | Type | Default | Description |\n");
        md.push_str("|--------|------|---------|-------------|\n");
        self.add_row(
            &mut md,
            "model",
            "string",
            "gpt-4o",
            "Model for retrieval (more capable model recommended)",
        );
        self.add_row(
            &mut md,
            "endpoint",
            "string",
            "https://api.openai.com/v1",
            "API endpoint",
        );
        self.add_row(
            &mut md,
            "api_key",
            "string?",
            "null",
            "API key (optional, uses default if not set)",
        );
        self.add_row(&mut md, "max_tokens", "usize", "100", "Maximum tokens for response");
        self.add_row(&mut md, "temperature", "f32", "0.0", "Temperature for generation");
        md.push_str("\n");

        // LLM.pilot section
        md.push_str("## `[llm.pilot]`\n\n");
        md.push_str("Pilot client - used for intelligent navigation guidance.\n\n");
        md.push_str("| Option | Type | Default | Description |\n");
        md.push_str("|--------|------|---------|-------------|\n");
        self.add_row(
            &mut md,
            "model",
            "string",
            "gpt-4o-mini",
            "Model for pilot navigation (fast model recommended)",
        );
        self.add_row(
            &mut md,
            "endpoint",
            "string",
            "https://api.openai.com/v1",
            "API endpoint",
        );
        self.add_row(
            &mut md,
            "api_key",
            "string?",
            "null",
            "API key (optional, uses default if not set)",
        );
        self.add_row(&mut md, "max_tokens", "usize", "300", "Maximum tokens for response");
        self.add_row(&mut md, "temperature", "f32", "0.0", "Temperature for generation");
        md.push_str("\n");

        // LLM.retry section
        md.push_str("## `[llm.retry]`\n\n");
        md.push_str("Retry configuration for all LLM calls.\n\n");
        md.push_str("| Option | Type | Default | Description |\n");
        md.push_str("|--------|------|---------|-------------|\n");
        self.add_row(&mut md, "max_attempts", "usize", "3", "Maximum retry attempts");
        self.add_row(
            &mut md,
            "initial_delay_ms",
            "u64",
            "500",
            "Initial delay before first retry (ms)",
        );
        self.add_row(
            &mut md,
            "max_delay_ms",
            "u64",
            "30000",
            "Maximum delay between retries (ms)",
        );
        self.add_row(
            &mut md,
            "multiplier",
            "f64",
            "2.0",
            "Multiplier for exponential backoff",
        );
        self.add_row(
            &mut md,
            "retry_on_rate_limit",
            "bool",
            "true",
            "Whether to retry on rate limit errors",
        );
        md.push_str("\n");

        // LLM.throttle section
        md.push_str("## `[llm.throttle]`\n\n");
        md.push_str("Throttle/rate limiting configuration for all LLM calls.\n\n");
        md.push_str("| Option | Type | Default | Description |\n");
        md.push_str("|--------|------|---------|-------------|\n");
        self.add_row(
            &mut md,
            "max_concurrent_requests",
            "usize",
            "10",
            "Maximum concurrent LLM API calls",
        );
        self.add_row(
            &mut md,
            "requests_per_minute",
            "usize",
            "500",
            "Rate limit: requests per minute",
        );
        self.add_row(&mut md, "enabled", "bool", "true", "Enable rate limiting");
        self.add_row(
            &mut md,
            "semaphore_enabled",
            "bool",
            "true",
            "Enable semaphore-based concurrency",
        );
        md.push_str("\n");

        // LLM.fallback section
        md.push_str("## `[llm.fallback]`\n\n");
        md.push_str("Fallback configuration for all LLM calls.\n\n");
        md.push_str("| Option | Type | Default | Description |\n");
        md.push_str("|--------|------|---------|-------------|\n");
        self.add_row(
            &mut md,
            "enabled",
            "bool",
            "true",
            "Enable fallback mechanism",
        );
        self.add_row(
            &mut md,
            "models",
            "[string]",
            "[\"gpt-4o-mini\", \"glm-4-flash\"]",
            "Fallback models in priority order",
        );
        self.add_row(
            &mut md,
            "endpoints",
            "[string]",
            "[]",
            "Fallback endpoints in priority order",
        );
        self.add_row(
            &mut md,
            "on_rate_limit",
            "string",
            "retry_then_fallback",
            "Behavior on rate limit (retry, fallback, retry_then_fallback, fail)",
        );
        self.add_row(
            &mut md,
            "on_timeout",
            "string",
            "retry_then_fallback",
            "Behavior on timeout",
        );
        self.add_row(
            &mut md,
            "on_all_failed",
            "string",
            "return_error",
            "Behavior when all attempts fail (return_error, return_cache)",
        );
        md.push_str("\n");

        // Metrics section
        md.push_str("## `[metrics]`\n\n");
        md.push_str("Unified metrics configuration for observability.\n\n");
        md.push_str("| Option | Type | Default | Description |\n");
        md.push_str("|--------|------|---------|-------------|\n");
        self.add_row(&mut md, "enabled", "bool", "true", "Enable metrics collection");
        self.add_row(
            &mut md,
            "storage_path",
            "string",
            "./workspace/metrics",
            "Storage path for persisted metrics",
        );
        self.add_row(
            &mut md,
            "retention_days",
            "usize",
            "30",
            "Retention period in days",
        );
        md.push_str("\n");

        // Metrics.llm section
        md.push_str("## `[metrics.llm]`\n\n");
        md.push_str("LLM-specific metrics configuration.\n\n");
        md.push_str("| Option | Type | Default | Description |\n");
        md.push_str("|--------|------|---------|-------------|\n");
        self.add_row(&mut md, "track_tokens", "bool", "true", "Track token usage");
        self.add_row(&mut md, "track_latency", "bool", "true", "Track latency");
        self.add_row(&mut md, "track_cost", "bool", "true", "Track estimated cost");
        self.add_row(
            &mut md,
            "cost_per_1k_input_tokens",
            "f64",
            "0.00015",
            "Cost per 1K input tokens (gpt-4o-mini)",
        );
        self.add_row(
            &mut md,
            "cost_per_1k_output_tokens",
            "f64",
            "0.0006",
            "Cost per 1K output tokens (gpt-4o-mini)",
        );
        md.push_str("\n");

        // Metrics.pilot section
        md.push_str("## `[metrics.pilot]`\n\n");
        md.push_str("Pilot-specific metrics configuration.\n\n");
        md.push_str("| Option | Type | Default | Description |\n");
        md.push_str("|--------|------|---------|-------------|\n");
        self.add_row(&mut md, "track_decisions", "bool", "true", "Track Pilot decisions");
        self.add_row(
            &mut md,
            "track_accuracy",
            "bool",
            "true",
            "Track decision accuracy (requires feedback)",
        );
        self.add_row(&mut md, "track_feedback", "bool", "true", "Track user feedback");
        md.push_str("\n");

        // Metrics.retrieval section
        md.push_str("## `[metrics.retrieval]`\n\n");
        md.push_str("Retrieval-specific metrics configuration.\n\n");
        md.push_str("| Option | Type | Default | Description |\n");
        md.push_str("|--------|------|---------|-------------|\n");
        self.add_row(&mut md, "track_paths", "bool", "true", "Track search paths");
        self.add_row(&mut md, "track_scores", "bool", "true", "Track relevance scores");
        self.add_row(&mut md, "track_iterations", "bool", "true", "Track iterations");
        self.add_row(&mut md, "track_cache", "bool", "true", "Track cache hits/misses");
        md.push_str("\n");

        // Pilot section
        md.push_str("## `[pilot]`\n\n");
        md.push_str("Pilot navigation configuration.\n\n");
        md.push_str("| Option | Type | Default | Description |\n");
        md.push_str("|--------|------|---------|-------------|\n");
        self.add_row(
            &mut md,
            "mode",
            "string",
            "Balanced",
            "Operation mode (Aggressive, Balanced, Conservative, AlgorithmOnly)",
        );
        self.add_row(
            &mut md,
            "guide_at_start",
            "bool",
            "true",
            "Whether to provide guidance at search start",
        );
        self.add_row(
            &mut md,
            "guide_at_backtrack",
            "bool",
            "true",
            "Whether to provide guidance during backtracking",
        );
        md.push_str("\n");

        // Pilot.budget section
        md.push_str("## `[pilot.budget]`\n\n");
        md.push_str("Token and call budget constraints.\n\n");
        md.push_str("| Option | Type | Default | Description |\n");
        md.push_str("|--------|------|---------|-------------|\n");
        self.add_row(
            &mut md,
            "max_tokens_per_query",
            "usize",
            "2000",
            "Maximum total tokens per query",
        );
        self.add_row(
            &mut md,
            "max_tokens_per_call",
            "usize",
            "500",
            "Maximum tokens per single LLM call",
        );
        self.add_row(
            &mut md,
            "max_calls_per_query",
            "usize",
            "5",
            "Maximum number of LLM calls per query",
        );
        self.add_row(
            &mut md,
            "max_calls_per_level",
            "usize",
            "2",
            "Maximum number of LLM calls per tree level",
        );
        self.add_row(
            &mut md,
            "hard_limit",
            "bool",
            "true",
            "Whether to enforce hard limits (true) or soft limits (false)",
        );
        md.push_str("\n");

        // Pilot.intervention section
        md.push_str("## `[pilot.intervention]`\n\n");
        md.push_str("Intervention threshold settings.\n\n");
        md.push_str("| Option | Type | Default | Description |\n");
        md.push_str("|--------|------|---------|-------------|\n");
        self.add_row(
            &mut md,
            "fork_threshold",
            "usize",
            "3",
            "Minimum candidates to trigger fork intervention",
        );
        self.add_row(
            &mut md,
            "score_gap_threshold",
            "f32",
            "0.15",
            "Score gap threshold (intervene when scores are close)",
        );
        self.add_row(
            &mut md,
            "low_score_threshold",
            "f32",
            "0.3",
            "Low score threshold (intervene when best score is below this)",
        );
        self.add_row(
            &mut md,
            "max_interventions_per_level",
            "usize",
            "2",
            "Maximum interventions allowed per tree level",
        );
        md.push_str("\n");

        // Pilot.feedback section
        md.push_str("## `[pilot.feedback]`\n\n");
        md.push_str("Feedback and learning configuration.\n\n");
        md.push_str("| Option | Type | Default | Description |\n");
        md.push_str("|--------|------|---------|-------------|\n");
        self.add_row(&mut md, "enabled", "bool", "true", "Enable feedback collection");
        self.add_row(
            &mut md,
            "storage_path",
            "string",
            "./workspace/feedback",
            "Storage path for feedback data",
        );
        self.add_row(
            &mut md,
            "learning_rate",
            "f32",
            "0.1",
            "Learning rate for feedback-based improvements",
        );
        self.add_row(
            &mut md,
            "min_samples_for_learning",
            "usize",
            "10",
            "Minimum samples before applying learning",
        );
        md.push_str("\n");

        // Retrieval section
        md.push_str("## `[retrieval]`\n\n");
        md.push_str("Retrieval model and behavior configuration.\n\n");
        md.push_str("| Option | Type | Default | Description |\n");
        md.push_str("|--------|------|---------|-------------|\n");
        self.add_row(
            &mut md,
            "model",
            "string",
            "gpt-4o",
            "Model for retrieval navigation",
        );
        self.add_row(
            &mut md,
            "endpoint",
            "string",
            "https://api.openai.com/v1",
            "API endpoint",
        );
        self.add_row(&mut md, "top_k", "usize", "3", "Number of top results to return");
        self.add_row(
            &mut md,
            "max_tokens",
            "usize",
            "1000",
            "Maximum tokens for retrieval context",
        );
        self.add_row(&mut md, "temperature", "f32", "0.0", "Temperature for retrieval");
        md.push_str("\n");

        // Retrieval.search section
        md.push_str("## `[retrieval.search]`\n\n");
        md.push_str("Search algorithm configuration.\n\n");
        md.push_str("| Option | Type | Default | Description |\n");
        md.push_str("|--------|------|---------|-------------|\n");
        self.add_row(&mut md, "top_k", "usize", "5", "Number of top-k results to return");
        self.add_row(
            &mut md,
            "beam_width",
            "usize",
            "3",
            "Beam width for multi-path search",
        );
        self.add_row(
            &mut md,
            "max_iterations",
            "usize",
            "10",
            "Maximum iterations for search algorithms",
        );
        self.add_row(
            &mut md,
            "min_score",
            "f32",
            "0.1",
            "Minimum score to include a path",
        );
        md.push_str("\n");

        // Retrieval.sufficiency section
        md.push_str("## `[retrieval.sufficiency]`\n\n");
        md.push_str("Sufficiency checker configuration.\n\n");
        md.push_str("| Option | Type | Default | Description |\n");
        md.push_str("|--------|------|---------|-------------|\n");
        self.add_row(
            &mut md,
            "min_tokens",
            "usize",
            "500",
            "Minimum tokens for sufficiency",
        );
        self.add_row(
            &mut md,
            "target_tokens",
            "usize",
            "2000",
            "Target tokens for full sufficiency",
        );
        self.add_row(
            &mut md,
            "max_tokens",
            "usize",
            "4000",
            "Maximum tokens before stopping",
        );
        self.add_row(
            &mut md,
            "min_content_length",
            "usize",
            "200",
            "Minimum content length (characters)",
        );
        self.add_row(
            &mut md,
            "confidence_threshold",
            "f32",
            "0.7",
            "Confidence threshold for LLM judge",
        );
        md.push_str("\n");

        // Retrieval.cache section
        md.push_str("## `[retrieval.cache]`\n\n");
        md.push_str("Cache configuration.\n\n");
        md.push_str("| Option | Type | Default | Description |\n");
        md.push_str("|--------|------|---------|-------------|\n");
        self.add_row(&mut md, "max_entries", "usize", "1000", "Maximum cache entries");
        self.add_row(&mut md, "ttl_secs", "u64", "3600", "Time-to-live in seconds");
        md.push_str("\n");

        // Retrieval.strategy section
        md.push_str("## `[retrieval.strategy]`\n\n");
        md.push_str("Strategy-specific configuration.\n\n");
        md.push_str("| Option | Type | Default | Description |\n");
        md.push_str("|--------|------|---------|-------------|\n");
        self.add_row(
            &mut md,
            "exploration_weight",
            "f32",
            "1.414",
            "MCTS exploration weight (√2)",
        );
        self.add_row(
            &mut md,
            "similarity_threshold",
            "f32",
            "0.5",
            "Semantic similarity threshold",
        );
        self.add_row(
            &mut md,
            "high_similarity_threshold",
            "f32",
            "0.8",
            "High similarity for 'answer' decision",
        );
        self.add_row(
            &mut md,
            "low_similarity_threshold",
            "f32",
            "0.3",
            "Low similarity for 'explore' decision",
        );
        md.push_str("\n");

        // Retrieval.content section
        md.push_str("## `[retrieval.content]`\n\n");
        md.push_str("Content aggregator configuration.\n\n");
        md.push_str("| Option | Type | Default | Description |\n");
        md.push_str("|--------|------|---------|-------------|\n");
        self.add_row(
            &mut md,
            "enabled",
            "bool",
            "true",
            "Enable content aggregator",
        );
        self.add_row(
            &mut md,
            "token_budget",
            "usize",
            "4000",
            "Maximum tokens for aggregated content",
        );
        self.add_row(
            &mut md,
            "min_relevance_score",
            "f32",
            "0.2",
            "Minimum relevance score threshold (0.0-1.0)",
        );
        self.add_row(
            &mut md,
            "scoring_strategy",
            "string",
            "hybrid",
            "Scoring strategy (keyword, bm25, hybrid)",
        );
        self.add_row(
            &mut md,
            "output_format",
            "string",
            "markdown",
            "Output format (markdown, json, tree, flat)",
        );
        self.add_row(
            &mut md,
            "include_scores",
            "bool",
            "false",
            "Include relevance scores in output",
        );
        self.add_row(
            &mut md,
            "hierarchical_min_per_level",
            "f32",
            "0.1",
            "Minimum budget allocation per depth level",
        );
        self.add_row(
            &mut md,
            "deduplicate",
            "bool",
            "true",
            "Enable content deduplication",
        );
        self.add_row(
            &mut md,
            "dedup_threshold",
            "f32",
            "0.9",
            "Similarity threshold for deduplication",
        );
        md.push_str("\n");

        // Retrieval.multiturn section
        md.push_str("## `[retrieval.multiturn]`\n\n");
        md.push_str("Multi-turn retrieval configuration.\n\n");
        md.push_str("| Option | Type | Default | Description |\n");
        md.push_str("|--------|------|---------|-------------|\n");
        self.add_row(
            &mut md,
            "enabled",
            "bool",
            "true",
            "Enable multi-turn retrieval",
        );
        self.add_row(
            &mut md,
            "max_sub_queries",
            "usize",
            "3",
            "Maximum sub-queries per query",
        );
        self.add_row(
            &mut md,
            "decomposition_model",
            "string",
            "gpt-4o-mini",
            "Model for query decomposition",
        );
        self.add_row(
            &mut md,
            "aggregation_strategy",
            "string",
            "merge",
            "Aggregation strategy (merge, rank, synthesize)",
        );
        md.push_str("\n");

        // Retrieval.reference section
        md.push_str("## `[retrieval.reference]`\n\n");
        md.push_str("Reference following configuration.\n\n");
        md.push_str("| Option | Type | Default | Description |\n");
        md.push_str("|--------|------|---------|-------------|\n");
        self.add_row(
            &mut md,
            "enabled",
            "bool",
            "true",
            "Enable reference following",
        );
        self.add_row(&mut md, "max_depth", "usize", "3", "Maximum reference depth");
        self.add_row(
            &mut md,
            "max_references",
            "usize",
            "10",
            "Maximum references to follow",
        );
        self.add_row(
            &mut md,
            "follow_pages",
            "bool",
            "true",
            "Follow page references",
        );
        self.add_row(
            &mut md,
            "follow_tables_figures",
            "bool",
            "true",
            "Follow table/figure references",
        );
        self.add_row(
            &mut md,
            "min_confidence",
            "f32",
            "0.5",
            "Minimum confidence to follow reference",
        );
        md.push_str("\n");

        // Storage section
        md.push_str("## `[storage]`\n\n");
        md.push_str("Storage configuration.\n\n");
        md.push_str("| Option | Type | Default | Description |\n");
        md.push_str("|--------|------|---------|-------------|\n");
        self.add_row(
            &mut md,
            "workspace_dir",
            "string",
            "./workspace",
            "Workspace directory for persisted documents",
        );
        self.add_row(&mut md, "cache_size", "usize", "100", "Cache size");
        self.add_row(
            &mut md,
            "atomic_writes",
            "bool",
            "true",
            "Enable atomic file writes",
        );
        self.add_row(&mut md, "file_lock", "bool", "true", "Enable file locking");
        self.add_row(
            &mut md,
            "checksum_enabled",
            "bool",
            "true",
            "Enable checksum verification",
        );
        md.push_str("\n");

        // Storage.compression section
        md.push_str("## `[storage.compression]`\n\n");
        md.push_str("Compression configuration.\n\n");
        md.push_str("| Option | Type | Default | Description |\n");
        md.push_str("|--------|------|---------|-------------|\n");
        self.add_row(&mut md, "enabled", "bool", "false", "Enable compression");
        self.add_row(
            &mut md,
            "algorithm",
            "string",
            "gzip",
            "Compression algorithm (gzip, zstd, lz4)",
        );
        self.add_row(&mut md, "level", "u32", "6", "Compression level");
        md.push_str("\n");

        // Indexer section
        md.push_str("## `[indexer]`\n\n");
        md.push_str("Controls document indexing behavior.\n\n");
        md.push_str("| Option | Type | Default | Description |\n");
        md.push_str("|--------|------|---------|-------------|\n");
        self.add_row(
            &mut md,
            "subsection_threshold",
            "usize",
            "300",
            "Word count threshold for splitting sections into subsections",
        );
        self.add_row(
            &mut md,
            "max_segment_tokens",
            "usize",
            "3000",
            "Maximum tokens to send in a single segmentation request",
        );
        self.add_row(
            &mut md,
            "max_summary_tokens",
            "usize",
            "200",
            "Maximum tokens for each summary",
        );
        self.add_row(
            &mut md,
            "min_summary_tokens",
            "usize",
            "20",
            "Minimum content tokens required to generate a summary",
        );
        md.push_str("\n");

        md
    }

    fn add_row(&self, md: &mut String, name: &str, ty: &str, default: &str, desc: &str) {
        md.push_str(&format!(
            "| `{}` | {} | {} | {} |\n",
            name, ty, default, desc
        ));
    }

    /// Generate an example TOML file with all options.
    pub fn to_example_toml(&self) -> String {
        toml::to_string_pretty(&self.config).unwrap_or_else(|e| {
            format!(
                "# Error generating TOML: {}\n\n# Using default config\n{}",
                e,
                Self::fallback_toml()
            )
        })
    }

    fn fallback_toml() -> String {
        r#"# Vectorless Configuration Example
# Copy this file to vectorless.toml and fill in your API keys
#
# All configuration is loaded from this file only.
# No environment variables are used - this ensures explicit, traceable configuration.

# ============================================================================
# LLM Configuration (Unified)
# ============================================================================
#
# The LLM pool allows configuring different models for different purposes:
# - summary: Used for generating document summaries during indexing
# - retrieval: Used for retrieval decisions and content evaluation
# - pilot: Used for intelligent navigation guidance
#
# Each client can have its own model, endpoint, and settings.

[llm]
# Default API key (used by all clients unless overridden per-client)
api_key = "sk-your-api-key-here"

# Summary client - generates document summaries during indexing
# Use a fast, cheap model for bulk processing
[llm.summary]
model = "gpt-4o-mini"
endpoint = "https://api.openai.com/v1"
max_tokens = 200
temperature = 0.0
# api_key = "sk-specific-key-for-summary"  # Optional: override default

# Retrieval client - used for retrieval decisions and content evaluation
# Can use a more capable model for better decisions
[llm.retrieval]
model = "gpt-4o"
endpoint = "https://api.openai.com/v1"
max_tokens = 100
temperature = 0.0
# api_key = "sk-specific-key-for-retrieval"  # Optional: override default

# Pilot client - used for intelligent navigation guidance
# Use a fast model for quick navigation decisions
[llm.pilot]
model = "gpt-4o-mini"
endpoint = "https://api.openai.com/v1"
max_tokens = 300
temperature = 0.0
# api_key = "sk-specific-key-for-pilot"  # Optional: override default

# Retry configuration (applies to all LLM calls)
[llm.retry]
max_attempts = 3
initial_delay_ms = 500
max_delay_ms = 30000
multiplier = 2.0
retry_on_rate_limit = true

# Throttle/rate limiting configuration (applies to all LLM calls)
[llm.throttle]
max_concurrent_requests = 10
requests_per_minute = 500
enabled = true
semaphore_enabled = true

# Fallback configuration (applies to all LLM calls)
[llm.fallback]
enabled = true
models = ["gpt-4o-mini", "glm-4-flash"]
on_rate_limit = "retry_then_fallback"
on_timeout = "retry_then_fallback"
on_all_failed = "return_error"

# ============================================================================
# Metrics Configuration (Unified)
# ============================================================================

[metrics]
enabled = true
storage_path = "./workspace/metrics"
retention_days = 30

[metrics.llm]
track_tokens = true
track_latency = true
track_cost = true
cost_per_1k_input_tokens = 0.00015   # gpt-4o-mini pricing
cost_per_1k_output_tokens = 0.0006

[metrics.pilot]
track_decisions = true
track_accuracy = true
track_feedback = true

[metrics.retrieval]
track_paths = true
track_scores = true
track_iterations = true
track_cache = true

# ============================================================================
# Pilot Configuration
# ============================================================================

[pilot]
mode = "Balanced"  # Aggressive | Balanced | Conservative | AlgorithmOnly
guide_at_start = true
guide_at_backtrack = true

[pilot.budget]
max_tokens_per_query = 2000
max_tokens_per_call = 500
max_calls_per_query = 5
max_calls_per_level = 2
hard_limit = true

[pilot.intervention]
fork_threshold = 3
score_gap_threshold = 0.15
low_score_threshold = 0.3
max_interventions_per_level = 2

[pilot.feedback]
enabled = true
storage_path = "./workspace/feedback"
learning_rate = 0.1
min_samples_for_learning = 10

# ============================================================================
# Retrieval Configuration
# ============================================================================

[retrieval]
model = "gpt-4o"
endpoint = "https://api.openai.com/v1"
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
scoring_strategy = "hybrid"  # keyword | bm25 | hybrid
output_format = "markdown"
include_scores = false
hierarchical_min_per_level = 0.1
deduplicate = true
dedup_threshold = 0.9

# ============================================================================
# Multi-turn Retrieval Configuration
# ============================================================================

[retrieval.multiturn]
enabled = true
max_sub_queries = 3
decomposition_model = "gpt-4o-mini"
aggregation_strategy = "merge"  # merge | rank | synthesize

# ============================================================================
# Reference Following Configuration
# ============================================================================

[retrieval.reference]
enabled = true
max_depth = 3
max_references = 10
follow_pages = true
follow_tables_figures = true
min_confidence = 0.5

# ============================================================================
# Storage Configuration
# ============================================================================

[storage]
workspace_dir = "./workspace"
cache_size = 100
atomic_writes = true
file_lock = true
checksum_enabled = true

[storage.compression]
enabled = false
algorithm = "gzip"
level = 6

# ============================================================================
# Indexer Configuration
# ============================================================================

[indexer]
subsection_threshold = 300
max_segment_tokens = 3000
max_summary_tokens = 200
min_summary_tokens = 20
"#
        .to_string()
    }

    /// Generate a minimal example TOML file.
    pub fn to_minimal_toml(&self) -> String {
        r#"# Minimal Vectorless Configuration
# Most options have sensible defaults

[llm]
api_key = "your-api-key-here"

[retrieval]
top_k = 5
"#
        .to_string()
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
        assert!(md.contains("## `[llm]`"));
        assert!(md.contains("## `[llm.summary]`"));
        assert!(md.contains("## `[metrics]`"));
        assert!(md.contains("## `[pilot]`"));
        assert!(md.contains("## `[retrieval]`"));
        assert!(md.contains("## `[retrieval.content]`"));
    }

    #[test]
    fn test_config_docs_toml() {
        let docs = ConfigDocs::with_defaults();
        let toml = docs.to_example_toml();

        assert!(toml.contains("[llm]") || toml.contains("[indexer]"));
    }

    #[test]
    fn test_config_docs_minimal_toml() {
        let docs = ConfigDocs::with_defaults();
        let toml = docs.to_minimal_toml();

        assert!(toml.contains("[llm]"));
        assert!(toml.len() < 200); // Should be minimal
    }
}
