// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Prompt builders for Pilot LLM calls.
//!
//! Provides specialized prompts for each intervention point:
//! - START: Search initialization guidance
//! - FORK: Branch selection at decision points
//! - BACKTRACK: Recovery after dead ends
//! - EVALUATE: Node relevance assessment

mod builder;
mod templates;

pub use builder::PromptBuilder;
pub use templates::{
    ForkPrompt, PromptTemplate, StartPrompt, BacktrackPrompt, EvaluatePrompt,
};
