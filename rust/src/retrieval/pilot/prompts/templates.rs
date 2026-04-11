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
    pub fn system_start() -> String {
        r#"You are a document navigation assistant. Help identify the best entry points for searching a hierarchical document.

CRITICAL: You MUST respond with ONLY valid JSON. No markdown code blocks, No explanation. Just the JSON object.

Your response must have this EXACT structure:
{
  "entry_points": ["Title 1", "Title 2"],
  "reasoning": "Brief explanation",
  "confidence": 0.85
}

Rules:
- entry_points: Array of node title strings (from the candidates provided)
- reasoning: String explaining your choice
- confidence: Number between 0.0 and 1.0 (use a number, NOT "high"/"medium"/"low")"#.to_string()
    }

    pub fn user_start() -> String {
        r#"{context}

Respond with ONLY the JSON object (no markdown, no explanation):
{
  "entry_points": ["list of node titles as strings"],
  "reasoning": "your reasoning here",
  "confidence": 0.85
}"#.to_string()
    }

    pub fn system_fork() -> String {
        r#"You are a document navigation assistant. At each decision point, rank the candidate branches by their likelihood of containing the answer to the user's query.

CRITICAL: You MUST respond with ONLY valid JSON. No markdown code blocks.

Your response must have this EXACT structure:
{
  "ranked_candidates": [
    {"index": 0, "score": 0.9, "reason": "explanation"}
  ],
  "direction": "go_deeper",
  "confidence": 0.85,
  "reasoning": "overall explanation"
}

Rules:
- ranked_candidates: Array of objects with index (number), score (0.0-1.0), reason (string)
- direction: One of "go_deeper", "explore_siblings", "backtrack", "found_answer"
- confidence: Number between 0.0 and 1.0 (NOT a string)"#.to_string()
    }

    pub fn user_fork() -> String {
        r#"{context}

Respond with ONLY the JSON object:
{
  "ranked_candidates": [
    {"index": 0, "score": 0.9, "reason": "why this candidate"}
  ],
  "direction": "go_deeper",
  "confidence": 0.85,
  "reasoning": "overall explanation"
}"#
            .to_string()
    }

    pub fn system_backtrack() -> String {
        r#"You are a document navigation assistant. When a search path fails to find the answer, analyze why and suggest alternative branches to explore.

CRITICAL: You MUST respond with ONLY valid JSON. No markdown code blocks.

Your response must have this EXACT structure:
{
  "alternative_branches": [
    {"index": 0, "score": 0.8, "reason": "explanation"}
  ],
  "direction": "backtrack",
  "confidence": 0.85,
  "reasoning": "why the original path failed"
}"#.to_string()
    }

    pub fn user_backtrack() -> String {
        r#"{context}

Respond with ONLY the JSON object:
{
  "alternative_branches": [
    {"index": 0, "score": 0.8, "reason": "why this alternative"}
  ],
  "direction": "backtrack",
  "confidence": 0.85,
  "reasoning": "why original path failed"
}"#.to_string()
    }

    pub fn system_evaluate() -> String {
        r#"You are a document analysis assistant. Evaluate whether the current node contains the answer to the user's query.

CRITICAL: You MUST respond with ONLY valid JSON. No markdown code blocks.

Your response must have this EXACT structure:
{
  "relevance_score": 0.85,
  "is_answer": false,
  "direction": "go_deeper",
  "confidence": 0.85,
  "reasoning": "explanation"
}"#.to_string()
    }

    pub fn user_evaluate() -> String {
        r#"{context}

Respond with ONLY the JSON object:
{
  "relevance_score": 0.85,
  "is_answer": false,
  "direction": "go_deeper",
  "confidence": 0.85,
  "reasoning": "explanation"
}"#
            .to_string()
    }

    pub fn system_locate_top3() -> String {
        r#"You are a document navigation assistant. Your task is to locate the most relevant sections in a document hierarchy for a user's query.

CRITICAL INSTRUCTIONS:
1. Analyze the user query carefully to understand the intent
2. Examine the provided Table of Contents (TOC) with node IDs
3. Select the TOP 3 most relevant nodes that would contain the answer
4. You MUST respond with ONLY valid JSON. No markdown code blocks. No explanations outside JSON.

Your response must have this EXACT structure:
{
  "reasoning": "Brief analysis of the query and why you selected these nodes",
  "candidates": [
    {"node_id": <number_from_toc>, "relevance_score": 0.95, "reason": "Why this node matches the query"},
    {"node_id": <number_from_toc>, "relevance_score": 0.80, "reason": "Why this node is also relevant"},
    {"node_id": <number_from_toc>, "relevance_score": 0.65, "reason": "Why this node might be relevant"}
  ]
}

Rules:
- node_id: MUST be a number from the provided TOC (copy exactly)
- relevance_score: Number between 0.0 and 1.0 (higher = more relevant)
- reason: Brief explanation for each selection
- candidates: Must have exactly 3 items, ordered by relevance (highest first)
- If fewer than 3 relevant nodes exist, use lower scores for less relevant ones"#.to_string()
    }

    pub fn user_locate_top3() -> String {
        r#"{context}

Based on the query and TOC above, select the TOP 3 most relevant nodes.

Respond with ONLY the JSON object:
{
  "reasoning": "Your analysis here",
  "candidates": [
    {"node_id": 1, "relevance_score": 0.95, "reason": "explanation"},
    {"node_id": 2, "relevance_score": 0.80, "reason": "explanation"},
    {"node_id": 3, "relevance_score": 0.65, "reason": "explanation"}
  ]
}"#
            .to_string()
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

impl LocateTop3Prompt {
    /// Get template with fallback.
    pub fn with_fallback() -> Self {
        Self {
            system: fallback::system_locate_top3(),
            template: fallback::user_locate_top3(),
        }
    }
}

/// Prompt template for LOCATE_TOP3 intervention point.
///
/// Used at the start to directly locate top-3 relevant nodes from TOC:
/// - Understand query intent
/// - Identify top 3 most relevant nodes with confidence scores
/// - Provide reasoning for each selection
#[derive(Debug, Clone)]
pub struct LocateTop3Prompt {
    system: String,
    template: String,
}

impl Default for LocateTop3Prompt {
    fn default() -> Self {
        Self::with_fallback()
    }
}

impl LocateTop3Prompt {
    /// Create a new locate top-3 prompt template.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create with custom templates.
    pub fn with_templates(system: String, template: String) -> Self {
        Self { system, template }
    }
}

impl PromptTemplate for LocateTop3Prompt {
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
  "reasoning": "Overall analysis of the query and document structure",
  "candidates": [
    {"node_id": 1, "relevance_score": 0.95, "reason": "Why this node is relevant"},
    {"node_id": 2, "relevance_score": 0.80, "reason": "Why this node is relevant"},
    {"node_id": 3, "relevance_score": 0.65, "reason": "Why this node is relevant"}
  ]
}"#
    }
}
