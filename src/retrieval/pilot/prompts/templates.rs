// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Prompt templates for different intervention points.
//!
//! Each template is designed for a specific decision context
//! and follows a consistent structure:
//! 1. System context (role definition)
//! 2. Task description
//! 3. Input format
//! 4. Output format (JSON schema)

use super::super::decision::InterventionPoint;

/// Common trait for prompt templates.
pub trait PromptTemplate: Send + Sync {
    /// Get the system prompt.
    fn system_prompt(&self) -> &str;

    /// Get the user prompt template.
    fn user_prompt_template(&self) -> &str;

    /// Get the intervention point this template is for.
    fn intervention_point(&self) -> InterventionPoint;

    /// Get the expected output format (JSON schema hint).
    fn output_format_hint(&self) -> &str;
}

/// Prompt template for START intervention point.
///
/// Used at the beginning of search to:
/// - Understand query intent
/// - Identify entry points
/// - Set search direction
#[derive(Debug, Clone)]
pub struct StartPrompt {
    system: String,
    template: String,
}

impl Default for StartPrompt {
    fn default() -> Self {
        Self::with_fallback()
    }
}

impl StartPrompt {
    /// Create a new start prompt template.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create with custom templates.
    pub fn with_templates(system: String, template: String) -> Self {
        Self { system, template }
    }
}

impl PromptTemplate for StartPrompt {
    fn system_prompt(&self) -> &str {
        &self.system
    }

    fn user_prompt_template(&self) -> &str {
        &self.template
    }

    fn intervention_point(&self) -> InterventionPoint {
        InterventionPoint::Start
    }

    fn output_format_hint(&self) -> &str {
        r#"{
  "entry_points": ["list of node titles to start from"],
  "reasoning": "explanation of why these entry points",
  "confidence": 0.0-1.0
}"#
    }
}

/// Prompt template for FORK intervention point.
///
/// Used when multiple candidate branches are available to:
/// - Rank candidates by relevance
/// - Recommend search direction
/// - Provide reasoning
#[derive(Debug, Clone)]
pub struct ForkPrompt {
    system: String,
    template: String,
}

impl Default for ForkPrompt {
    fn default() -> Self {
        Self::with_fallback()
    }
}

impl ForkPrompt {
    /// Create a new fork prompt template.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create with custom templates.
    pub fn with_templates(system: String, template: String) -> Self {
        Self { system, template }
    }
}

impl PromptTemplate for ForkPrompt {
    fn system_prompt(&self) -> &str {
        &self.system
    }

    fn user_prompt_template(&self) -> &str {
        &self.template
    }

    fn intervention_point(&self) -> InterventionPoint {
        InterventionPoint::Fork
    }

    fn output_format_hint(&self) -> &str {
        r#"{
  "ranked_candidates": [
    {"index": 0, "score": 0.9, "reason": "why this candidate"}
  ],
  "direction": "go_deeper|explore_siblings|backtrack|found_answer",
  "confidence": 0.0-1.0,
  "reasoning": "overall explanation"
}"#
    }
}

/// Prompt template for BACKTRACK intervention point.
///
/// Used when search needs to recover from a dead end to:
/// - Analyze failure reason
/// - Suggest alternative branches
/// - Guide recovery strategy
#[derive(Debug, Clone)]
pub struct BacktrackPrompt {
    system: String,
    template: String,
}

impl Default for BacktrackPrompt {
    fn default() -> Self {
        Self::with_fallback()
    }
}

impl BacktrackPrompt {
    /// Create a new backtrack prompt template.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create with custom templates.
    pub fn with_templates(system: String, template: String) -> Self {
        Self { system, template }
    }
}

impl PromptTemplate for BacktrackPrompt {
    fn system_prompt(&self) -> &str {
        &self.system
    }

    fn user_prompt_template(&self) -> &str {
        &self.template
    }

    fn intervention_point(&self) -> InterventionPoint {
        InterventionPoint::Backtrack
    }

    fn output_format_hint(&self) -> &str {
        r#"{
  "alternative_branches": [
    {"index": 0, "score": 0.8, "reason": "why this alternative"}
  ],
  "direction": "backtrack",
  "confidence": 0.0-1.0,
  "reasoning": "why the original path failed and alternatives chosen"
}"#
    }
}

/// Prompt template for EVALUATE intervention point.
///
/// Used to assess if a node contains the answer to:
/// - Determine relevance score
/// - Check if answer is found
/// - Guide further search
#[derive(Debug, Clone)]
pub struct EvaluatePrompt {
    system: String,
    template: String,
}

impl Default for EvaluatePrompt {
    fn default() -> Self {
        Self::with_fallback()
    }
}

impl EvaluatePrompt {
    /// Create a new evaluate prompt template.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create with custom templates.
    pub fn with_templates(system: String, template: String) -> Self {
        Self { system, template }
    }
}

impl PromptTemplate for EvaluatePrompt {
    fn system_prompt(&self) -> &str {
        &self.system
    }

    fn user_prompt_template(&self) -> &str {
        &self.template
    }

    fn intervention_point(&self) -> InterventionPoint {
        InterventionPoint::Evaluate
    }

    fn output_format_hint(&self) -> &str {
        r#"{
  "relevance_score": 0.0-1.0,
  "is_answer": true|false,
  "direction": "go_deeper|found_answer",
  "confidence": 0.0-1.0,
  "reasoning": "why this node is or isn't the answer"
}"#
    }
}

/// Fallback templates when file loading fails.
pub mod fallback {
    use super::*;

    pub fn system_start() -> String {
        "You are a document navigation assistant. Help identify the best starting point for searching a hierarchical document.".to_string()
    }

    pub fn user_start() -> String {
        r#"Given the following document structure and user query, identify the best entry points for search.

{context}

Respond in JSON format with your analysis."#.to_string()
    }

    pub fn system_fork() -> String {
        "You are a document navigation assistant. At each decision point, rank the candidate branches by their likelihood of containing the answer to the user's query.".to_string()
    }

    pub fn user_fork() -> String {
        r#"Given the current search context and candidate branches, rank them by relevance.

{context}

Respond in JSON format with ranked candidates."#.to_string()
    }

    pub fn system_backtrack() -> String {
        "You are a document navigation assistant. When a search path fails to find the answer, analyze why and suggest alternative branches to explore.".to_string()
    }

    pub fn user_backtrack() -> String {
        r#"The current search path did not find the answer. Analyze the failure and suggest alternatives.

{context}

Respond in JSON format with alternative branches."#.to_string()
    }

    pub fn system_evaluate() -> String {
        "You are a document analysis assistant. Evaluate whether the current node contains the answer to the user's query.".to_string()
    }

    pub fn user_evaluate() -> String {
        r#"Evaluate if this node contains the answer to the user's query.

{context}

Respond in JSON format with your evaluation."#.to_string()
    }
}

impl StartPrompt {
    /// Get template with fallback.
    pub fn with_fallback() -> Self {
        Self {
            system: fallback::system_start(),
            template: fallback::user_start(),
        }
    }
}

impl ForkPrompt {
    /// Get template with fallback.
    pub fn with_fallback() -> Self {
        Self {
            system: fallback::system_fork(),
            template: fallback::user_fork(),
        }
    }
}

impl BacktrackPrompt {
    /// Get template with fallback.
    pub fn with_fallback() -> Self {
        Self {
            system: fallback::system_backtrack(),
            template: fallback::user_backtrack(),
        }
    }
}

impl EvaluatePrompt {
    /// Get template with fallback.
    pub fn with_fallback() -> Self {
        Self {
            system: fallback::system_evaluate(),
            template: fallback::user_evaluate(),
        }
    }
}
