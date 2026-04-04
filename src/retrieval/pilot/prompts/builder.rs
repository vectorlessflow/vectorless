// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Prompt builder for constructing LLM prompts.
//!
//! Combines templates with context to produce final prompts.

use super::super::builder::PilotContext;
use super::super::decision::InterventionPoint;
use super::templates::{ForkPrompt, PromptTemplate, StartPrompt, BacktrackPrompt, EvaluatePrompt};

/// Built prompt ready for LLM call.
#[derive(Debug, Clone)]
pub struct BuiltPrompt {
    /// System prompt.
    pub system: String,
    /// User prompt.
    pub user: String,
    /// Total estimated tokens.
    pub estimated_tokens: usize,
}

/// Builder for constructing LLM prompts.
///
/// Manages prompt templates and constructs final prompts
/// by combining templates with context.
///
/// # Example
///
/// ```rust,ignore
/// use vectorless::retrieval::pilot::prompts::PromptBuilder;
///
/// let builder = PromptBuilder::new();
/// let prompt = builder.build(InterventionPoint::Fork, &context);
/// println!("System: {}", prompt.system);
/// println!("User: {}", prompt.user);
/// ```
pub struct PromptBuilder {
    start_template: StartPrompt,
    fork_template: ForkPrompt,
    backtrack_template: BacktrackPrompt,
    evaluate_template: EvaluatePrompt,
}

impl Default for PromptBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl PromptBuilder {
    /// Create a new prompt builder with default templates.
    pub fn new() -> Self {
        Self {
            start_template: StartPrompt::with_fallback(),
            fork_template: ForkPrompt::with_fallback(),
            backtrack_template: BacktrackPrompt::with_fallback(),
            evaluate_template: EvaluatePrompt::with_fallback(),
        }
    }

    /// Create with custom templates.
    pub fn with_templates(
        start: StartPrompt,
        fork: ForkPrompt,
        backtrack: BacktrackPrompt,
        evaluate: EvaluatePrompt,
    ) -> Self {
        Self {
            start_template: start,
            fork_template: fork,
            backtrack_template: backtrack,
            evaluate_template: evaluate,
        }
    }

    /// Build a prompt for the given intervention point.
    pub fn build(&self, point: InterventionPoint, context: &PilotContext) -> BuiltPrompt {
        match point {
            InterventionPoint::Start => self.build_start(context),
            InterventionPoint::Fork => self.build_fork(context),
            InterventionPoint::Backtrack => self.build_backtrack(context),
            InterventionPoint::Evaluate => self.build_evaluate(context),
        }
    }

    /// Build START prompt.
    fn build_start(&self, context: &PilotContext) -> BuiltPrompt {
        let template = &self.start_template;
        let system = template.system_prompt().to_string();
        let user = self.fill_template(template.user_prompt_template(), context);
        let estimated_tokens = self.estimate_tokens(&system) + self.estimate_tokens(&user);

        BuiltPrompt {
            system,
            user,
            estimated_tokens,
        }
    }

    /// Build FORK prompt.
    fn build_fork(&self, context: &PilotContext) -> BuiltPrompt {
        let template = &self.fork_template;
        let system = template.system_prompt().to_string();
        let user = self.fill_template(template.user_prompt_template(), context);
        let estimated_tokens = self.estimate_tokens(&system) + self.estimate_tokens(&user);

        BuiltPrompt {
            system,
            user,
            estimated_tokens,
        }
    }

    /// Build BACKTRACK prompt.
    fn build_backtrack(&self, context: &PilotContext) -> BuiltPrompt {
        let template = &self.backtrack_template;
        let system = template.system_prompt().to_string();
        let user = self.fill_template(template.user_prompt_template(), context);
        let estimated_tokens = self.estimate_tokens(&system) + self.estimate_tokens(&user);

        BuiltPrompt {
            system,
            user,
            estimated_tokens,
        }
    }

    /// Build EVALUATE prompt.
    fn build_evaluate(&self, context: &PilotContext) -> BuiltPrompt {
        let template = &self.evaluate_template;
        let system = template.system_prompt().to_string();
        let user = self.fill_template(template.user_prompt_template(), context);
        let estimated_tokens = self.estimate_tokens(&system) + self.estimate_tokens(&user);

        BuiltPrompt {
            system,
            user,
            estimated_tokens,
        }
    }

    /// Fill template with context.
    fn fill_template(&self, template: &str, context: &PilotContext) -> String {
        let mut result = template.to_string();

        // Replace context placeholder with full context
        result = result.replace("{context}", &context.to_string());

        // Replace individual sections
        result = result.replace("{query}", &context.query_section);
        result = result.replace("{path}", &context.path_section);
        result = result.replace("{candidates}", &context.candidates_section);
        result = result.replace("{toc}", &context.toc_section);

        result
    }

    /// Estimate token count for a string.
    fn estimate_tokens(&self, text: &str) -> usize {
        let char_count = text.chars().count();
        let chinese_count = text
            .chars()
            .filter(|c| ('\u{4E00}'..='\u{9FFF}').contains(c))
            .count();
        let english_count = char_count - chinese_count;

        (chinese_count as f32 / 1.5 + english_count as f32 / 4.0).ceil() as usize
    }

    /// Get the template for an intervention point.
    pub fn get_template(&self, point: InterventionPoint) -> &dyn PromptTemplate {
        match point {
            InterventionPoint::Start => &self.start_template,
            InterventionPoint::Fork => &self.fork_template,
            InterventionPoint::Backtrack => &self.backtrack_template,
            InterventionPoint::Evaluate => &self.evaluate_template,
        }
    }

    /// Get output format hint for an intervention point.
    pub fn output_format(&self, point: InterventionPoint) -> &'static str {
        match point {
            InterventionPoint::Start => {
                r#"{
  "entry_points": ["list of starting node titles"],
  "reasoning": "explanation",
  "confidence": 0.0-1.0
}"#
            }
            InterventionPoint::Fork => {
                r#"{
  "ranked_candidates": [
    {"index": 0, "score": 0.9, "reason": "explanation"}
  ],
  "direction": "go_deeper|explore_siblings|backtrack|found_answer",
  "confidence": 0.0-1.0,
  "reasoning": "explanation"
}"#
            }
            InterventionPoint::Backtrack => {
                r#"{
  "alternative_branches": [
    {"index": 0, "score": 0.8, "reason": "explanation"}
  ],
  "direction": "backtrack",
  "confidence": 0.0-1.0,
  "reasoning": "explanation"
}"#
            }
            InterventionPoint::Evaluate => {
                r#"{
  "relevance_score": 0.0-1.0,
  "is_answer": true|false,
  "direction": "go_deeper|found_answer",
  "confidence": 0.0-1.0,
  "reasoning": "explanation"
}"#
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_prompt_builder_creation() {
        let builder = PromptBuilder::new();
        assert!(!builder.start_template.system_prompt().is_empty());
        assert!(!builder.fork_template.system_prompt().is_empty());
    }

    #[test]
    fn test_build_fork_prompt() {
        let builder = PromptBuilder::new();
        let context = PilotContext {
            query_section: "Query: test query\n".to_string(),
            path_section: "Path: Root → Test\n".to_string(),
            candidates_section: "Candidates:\n1. Option A\n".to_string(),
            toc_section: String::new(),
            estimated_tokens: 50,
        };

        let prompt = builder.build(InterventionPoint::Fork, &context);

        assert!(!prompt.system.is_empty());
        assert!(!prompt.user.is_empty());
        assert!(prompt.user.contains("test query") || prompt.user.contains("Query"));
    }

    #[test]
    fn test_build_start_prompt() {
        let builder = PromptBuilder::new();
        let context = PilotContext {
            query_section: "Query: how to configure\n".to_string(),
            path_section: String::new(),
            candidates_section: String::new(),
            toc_section: "TOC:\n1. Config\n".to_string(),
            estimated_tokens: 30,
        };

        let prompt = builder.build(InterventionPoint::Start, &context);

        assert!(!prompt.system.is_empty());
        assert!(prompt.estimated_tokens > 0);
    }

    #[test]
    fn test_output_format() {
        let builder = PromptBuilder::new();

        let fork_format = builder.output_format(InterventionPoint::Fork);
        assert!(fork_format.contains("ranked_candidates"));

        let start_format = builder.output_format(InterventionPoint::Start);
        assert!(start_format.contains("entry_points"));
    }

    #[test]
    fn test_template_fallback() {
        let start = StartPrompt::with_fallback();
        assert!(!start.system_prompt().is_empty());
        assert!(!start.user_prompt_template().is_empty());

        let fork = ForkPrompt::with_fallback();
        assert!(!fork.system_prompt().is_empty());
    }
}
