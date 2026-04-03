// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Pipeline orchestrator for managing and executing index stages.
//!
//! The orchestrator provides:
//! - Stage registration with priority
//! - Dependency-based ordering
//! - Custom stage support
//!
//! # Example
//!
//! ```rust,ignore
//! use vectorless::core::index::pipeline::PipelineOrchestrator;
//! use vectorless::core::index::stages::{ParseStage, BuildStage};
//!
//! let orchestrator = PipelineOrchestrator::new()
//!     .stage(ParseStage::new())
//!     .stage(BuildStage::new())
//!     .stage(MyCustomStage::new());
//!
//! let result = orchestrator.execute(input, options).await?;
//! ```

use std::collections::HashMap;
use std::time::Instant;
use tracing::{info, warn, error};

use crate::core::Result;

use super::context::{IndexContext, IndexInput, IndexResult, StageResult};
use super::super::stages::IndexStage;
use super::super::PipelineOptions;

/// Stage entry with metadata for orchestration.
struct StageEntry {
    /// The stage implementation.
    stage: Box<dyn IndexStage>,
    /// Priority (lower = earlier execution).
    priority: i32,
    /// Names of stages this depends on.
    depends_on: Vec<String>,
}

impl std::fmt::Debug for StageEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StageEntry")
            .field("name", &self.stage.name())
            .field("priority", &self.priority)
            .field("depends_on", &self.depends_on)
            .finish()
    }
}

/// Pipeline orchestrator for stage management and execution.
///
/// Provides flexible stage registration with:
/// - Priority-based ordering
/// - Dependency resolution
/// - Custom stage support
///
/// # Stage Ordering
///
/// Stages are ordered by:
/// 1. Dependencies (must run after dependencies)
/// 2. Priority (lower = earlier)
/// 3. Registration order (tie-breaker)
///
/// # Example
///
/// ```rust,ignore
/// // Default pipeline
/// let orchestrator = PipelineOrchestrator::default();
///
/// // Custom pipeline
/// let orchestrator = PipelineOrchestrator::new()
///     .stage(ParseStage::new())
///     .stage_with_priority(MyAnalysisStage::new(), 50)  // Run after build (priority 20)
///     .stage_with_priority(BuildStage::new(), 20);
/// ```
pub struct PipelineOrchestrator {
    /// Registered stages with metadata.
    stages: Vec<StageEntry>,
}

impl Default for PipelineOrchestrator {
    fn default() -> Self {
        Self::new()
    }
}

impl PipelineOrchestrator {
    /// Create a new empty orchestrator.
    pub fn new() -> Self {
        Self { stages: Vec::new() }
    }

    /// Add a stage with default priority (100) and no dependencies.
    pub fn stage<S>(mut self, stage: S) -> Self
    where
        S: IndexStage + 'static,
    {
        self.stages.push(StageEntry {
            stage: Box::new(stage),
            priority: 100,
            depends_on: Vec::new(),
        });
        self
    }

    /// Add a stage with custom priority.
    ///
    /// Lower priority = earlier execution.
    /// Default priority is 100.
    pub fn stage_with_priority<S>(mut self, stage: S, priority: i32) -> Self
    where
        S: IndexStage + 'static,
    {
        self.stages.push(StageEntry {
            stage: Box::new(stage),
            priority,
            depends_on: Vec::new(),
        });
        self
    }

    /// Add a stage with priority and dependencies.
    ///
    /// The stage will run after all specified dependencies.
    pub fn stage_with_deps<S>(
        mut self,
        stage: S,
        priority: i32,
        depends_on: &[&str],
    ) -> Self
    where
        S: IndexStage + 'static,
    {
        self.stages.push(StageEntry {
            stage: Box::new(stage),
            priority,
            depends_on: depends_on.iter().map(|s| s.to_string()).collect(),
        });
        self
    }

    /// Remove all stages with the given name.
    pub fn remove_stage(mut self, name: &str) -> Self {
        self.stages.retain(|entry| entry.stage.name() != name);
        self
    }

    /// Check if a stage with the given name exists.
    pub fn has_stage(&self, name: &str) -> bool {
        self.stages.iter().any(|entry| entry.stage.name() == name)
    }

    /// Get the number of registered stages.
    pub fn stage_count(&self) -> usize {
        self.stages.len()
    }

    /// Resolve dependencies and return stage indices in execution order.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - A dependency refers to a non-existent stage
    /// - There's a circular dependency
    fn resolve_order(&self) -> Result<Vec<usize>> {
        // Build name -> index map
        let name_to_idx: HashMap<&str, usize> = self
            .stages
            .iter()
            .enumerate()
            .map(|(i, entry)| (entry.stage.name(), i))
            .collect();

        // Validate dependencies
        for entry in &self.stages {
            for dep in &entry.depends_on {
                if !name_to_idx.contains_key(dep.as_str()) {
                    return Err(crate::core::Error::Config(format!(
                        "Stage '{}' depends on non-existent stage '{}'",
                        entry.stage.name(),
                        dep
                    )));
                }
            }
        }

        // Topological sort with priority consideration (Kahn's algorithm)
        let n = self.stages.len();
        let mut in_degree: Vec<usize> = vec![0; n];
        let mut adjacency: HashMap<usize, Vec<usize>> = HashMap::new();

        for (i, entry) in self.stages.iter().enumerate() {
            for dep in &entry.depends_on {
                if let Some(&dep_idx) = name_to_idx.get(dep.as_str()) {
                    adjacency.entry(dep_idx).or_default().push(i);
                    in_degree[i] += 1;
                }
            }
        }

        // Collect stages with no dependencies, sorted by priority
        let mut ready: Vec<usize> = (0..n)
            .filter(|&i| in_degree[i] == 0)
            .collect();
        ready.sort_by_key(|&i| (self.stages[i].priority, i));

        let mut result: Vec<usize> = Vec::new();

        while let Some(idx) = ready.first().cloned() {
            ready.remove(0);
            result.push(idx);

            if let Some(neighbors) = adjacency.get(&idx) {
                for &neighbor in neighbors {
                    in_degree[neighbor] -= 1;
                    if in_degree[neighbor] == 0 {
                        // Insert in priority order
                        let entry = &self.stages[neighbor];
                        let pos = ready
                            .binary_search_by_key(&(entry.priority, neighbor), |&i| {
                                (self.stages[i].priority, i)
                            })
                            .unwrap_or_else(|e| e);
                        ready.insert(pos, neighbor);
                    }
                }
            }
        }

        // Check for cycles
        if result.len() != n {
            let remaining: Vec<&str> = result
                .iter()
                .filter(|&&i| !result.contains(&i))
                .map(|&i| self.stages[i].stage.name())
                .collect();
            return Err(crate::core::Error::Config(format!(
                "Circular dependency detected involving stages: {:?}",
                remaining
            )));
        }

        Ok(result)
    }

    /// Execute the pipeline.
    ///
    /// Stages are executed in dependency-resolved order.
    pub async fn execute(
        &mut self,
        input: IndexInput,
        options: PipelineOptions,
    ) -> Result<IndexResult> {
        let total_start = Instant::now();
        info!("Starting orchestrated pipeline with {} stages", self.stages.len());

        // Resolve execution order
        let order = self.resolve_order()?;
        let stage_names: Vec<&str> = order
            .iter()
            .map(|&i| self.stages[i].stage.name())
            .collect();
        info!("Execution order: {:?}", stage_names);

        // Create context
        let mut ctx = IndexContext::new(input, options);

        // Execute each stage in order
        for &idx in &order {
            let entry = &mut self.stages[idx];
            let stage_name = entry.stage.name().to_string();
            let is_optional = entry.stage.is_optional();
            info!("Executing stage: {} (priority {})", stage_name, entry.priority);

            match entry.stage.execute(&mut ctx).await {
                Ok(result) => {
                    ctx.stage_results.insert(stage_name.clone(), result);
                }
                Err(e) => {
                    if is_optional {
                        warn!("Optional stage {} failed: {}", stage_name, e);
                    } else {
                        error!("Required stage {} failed: {}", stage_name, e);
                        return Err(e);
                    }
                }
            }
        }

        let total_duration = total_start.elapsed().as_millis() as u64;
        info!(
            "Orchestrated pipeline completed in {}ms for document {}",
            total_duration, ctx.name
        );

        // Finalize result
        Ok(ctx.finalize())
    }

    /// Get list of stage names in execution order.
    pub fn stage_names(&self) -> Result<Vec<&str>> {
        let order = self.resolve_order()?;
        Ok(order.iter().map(|&i| self.stages[i].stage.name()).collect())
    }
}

/// Builder for creating custom stage configurations.
///
/// This is a convenience type for configuring custom stages
/// without manually calling the orchestrator methods.
pub struct CustomStageBuilder {
    name: String,
    priority: i32,
    depends_on: Vec<String>,
    optional: bool,
}

impl CustomStageBuilder {
    /// Create a new custom stage builder.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            priority: 100,
            depends_on: Vec::new(),
            optional: false,
        }
    }

    /// Set priority (lower = earlier).
    pub fn priority(mut self, priority: i32) -> Self {
        self.priority = priority;
        self
    }

    /// Add a dependency.
    pub fn depends_on(mut self, stage: impl Into<String>) -> Self {
        self.depends_on.push(stage.into());
        self
    }

    /// Mark as optional (failures won't stop pipeline).
    pub fn optional(mut self) -> Self {
        self.optional = true;
        self
    }

    /// Get the stage name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Get the priority.
    pub fn get_priority(&self) -> i32 {
        self.priority
    }

    /// Get dependencies.
    pub fn get_deps(&self) -> &[String] {
        &self.depends_on
    }

    /// Check if optional.
    pub fn is_optional(&self) -> bool {
        self.optional
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_orchestrator_creation() {
        let orchestrator = PipelineOrchestrator::new();
        assert_eq!(orchestrator.stage_count(), 0);
    }

    #[test]
    fn test_add_stages() {
        let orchestrator = PipelineOrchestrator::new()
            .stage_with_priority(MockStage::new("a"), 10)
            .stage_with_priority(MockStage::new("b"), 20)
            .stage_with_priority(MockStage::new("c"), 5);

        assert_eq!(orchestrator.stage_count(), 3);

        let names = orchestrator.stage_names().unwrap();
        assert_eq!(names, vec!["c", "a", "b"]); // priority order
    }

    #[test]
    fn test_dependency_resolution() {
        let orchestrator = PipelineOrchestrator::new()
            .stage_with_priority(MockStage::new("a"), 10)
            .stage_with_deps(MockStage::new("b"), 5, &["a"]) // b depends on a
            .stage_with_deps(MockStage::new("c"), 1, &["b"]); // c depends on b

        let names = orchestrator.stage_names().unwrap();
        assert_eq!(names, vec!["a", "b", "c"]);
    }

    #[test]
    fn test_missing_dependency() {
        let orchestrator = PipelineOrchestrator::new()
            .stage_with_deps(MockStage::new("a"), 10, &["nonexistent"]);

        let result = orchestrator.stage_names();
        assert!(result.is_err());
    }

    #[test]
    fn test_remove_stage() {
        let orchestrator = PipelineOrchestrator::new()
            .stage(MockStage::new("a"))
            .stage(MockStage::new("b"))
            .remove_stage("a");

        assert_eq!(orchestrator.stage_count(), 1);
        assert!(!orchestrator.has_stage("a"));
        assert!(orchestrator.has_stage("b"));
    }

    #[test]
    fn test_custom_stage_builder() {
        let builder = CustomStageBuilder::new("my_stage")
            .priority(50)
            .depends_on("parse")
            .optional();

        assert_eq!(builder.name(), "my_stage");
        assert_eq!(builder.get_priority(), 50);
        assert_eq!(builder.get_deps(), &["parse".to_string()]);
        assert!(builder.is_optional());
    }

    /// Mock stage for testing.
    struct MockStage {
        name: String,
    }

    impl MockStage {
        fn new(name: &str) -> Self {
            Self { name: name.to_string() }
        }
    }

    #[async_trait::async_trait]
    impl IndexStage for MockStage {
        fn name(&self) -> &str {
            &self.name
        }

        async fn execute(
            &mut self,
            _ctx: &mut IndexContext,
        ) -> Result<StageResult> {
            Ok(StageResult::success(&self.name))
        }
    }
}
