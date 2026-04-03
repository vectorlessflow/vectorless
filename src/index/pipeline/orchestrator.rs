// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Pipeline orchestrator for managing and executing index stages.
//!
//! The orchestrator provides:
//! - Stage registration with priority
//! - Dependency-based ordering via topological sort
//! - Failure policies (Fail, Skip, Retry)
//! - Execution groups for parallel execution
//!
//! # Example
//!
//! ```rust,ignore
//! use vectorless::index::pipeline::PipelineOrchestrator;
//! use vectorless::index::stages::{ParseStage, BuildStage};
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

use crate::domain::Result;

use super::context::{IndexContext, IndexInput, IndexResult, StageResult};
use super::policy::FailurePolicy;
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

/// Group of stages at the same dependency level (can run in parallel).
#[derive(Debug, Clone)]
pub struct ExecutionGroup {
    /// Indices of stages in this group.
    pub stage_indices: Vec<usize>,
    /// Whether this group has multiple stages (parallelizable).
    pub parallel: bool,
}

/// Pipeline orchestrator for stage management and execution.
///
/// Provides flexible stage registration with:
/// - Priority-based ordering
/// - Dependency resolution
/// - Failure policies (Fail, Skip, Retry)
/// - Execution groups for parallel execution
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

    /// Add a stage with default priority (100).
    ///
    /// Dependencies are automatically read from the stage's `depends_on()` method.
    pub fn stage<S>(mut self, stage: S) -> Self
    where
        S: IndexStage + 'static,
    {
        let deps = stage.depends_on();
        self.stages.push(StageEntry {
            stage: Box::new(stage),
            priority: 100,
            depends_on: deps.into_iter().map(|s| s.to_string()).collect(),
        });
        self
    }

    /// Add a stage with custom priority.
    ///
    /// Dependencies are automatically read from the stage's `depends_on()` method.
    /// Lower priority = earlier execution.
    /// Default priority is 100.
    pub fn stage_with_priority<S>(mut self, stage: S, priority: i32) -> Self
    where
        S: IndexStage + 'static,
    {
        let deps = stage.depends_on();
        self.stages.push(StageEntry {
            stage: Box::new(stage),
            priority,
            depends_on: deps.into_iter().map(|s| s.to_string()).collect(),
        });
        self
    }

    /// Add a stage with priority and explicit dependencies.
    ///
    /// Merges trait-level dependencies with explicitly provided ones.
    /// The stage will run after all specified dependencies.
    pub fn stage_with_deps<S>(
        mut self,
        stage: S,
        priority: i32,
        explicit_depends_on: &[&str],
    ) -> Self
    where
        S: IndexStage + 'static,
    {
        let trait_deps = stage.depends_on();
        let mut all_deps: Vec<String> = trait_deps
            .into_iter()
            .map(|s| s.to_string())
            .collect();

        // Add explicit deps that aren't already included
        for dep in explicit_depends_on {
            if !all_deps.iter().any(|d| d == dep) {
                all_deps.push(dep.to_string());
            }
        }

        self.stages.push(StageEntry {
            stage: Box::new(stage),
            priority,
            depends_on: all_deps,
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
                    return Err(crate::domain::Error::Config(format!(
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
            return Err(crate::domain::Error::Config(format!(
                "Circular dependency detected involving stages: {:?}",
                remaining
            )));
        }

        Ok(result)
    }

    /// Compute execution groups from resolved order.
    ///
    /// Stages with the same "level" in the dependency graph and no
    /// inter-dependencies can run in parallel.
    fn compute_execution_groups(&self, order: &[usize]) -> Vec<ExecutionGroup> {
        if order.is_empty() {
            return Vec::new();
        }

        // Build name -> index map
        let name_to_idx: HashMap<&str, usize> = self
            .stages
            .iter()
            .enumerate()
            .map(|(i, entry)| (entry.stage.name(), i))
            .collect();

        // Calculate level for each stage based on dependencies
        let mut levels: HashMap<usize, usize> = HashMap::new();

        for &idx in order {
            let entry = &self.stages[idx];
            let level = if entry.depends_on.is_empty() {
                0
            } else {
                entry
                    .depends_on
                    .iter()
                    .filter_map(|dep| {
                        name_to_idx
                            .get(dep.as_str())
                            .and_then(|&dep_idx| levels.get(&dep_idx))
                    })
                    .max()
                    .map(|&l| l + 1)
                    .unwrap_or(0)
            };
            levels.insert(idx, level);
        }

        // Group stages by level
        let mut level_groups: HashMap<usize, Vec<usize>> = HashMap::new();
        for &idx in order {
            let level = levels[&idx];
            level_groups.entry(level).or_default().push(idx);
        }

        // Convert to execution groups
        let max_level = *levels.values().max().unwrap_or(&0);
        (0..=max_level)
            .filter_map(|level| {
                level_groups.get(&level).map(|indices| ExecutionGroup {
                    stage_indices: indices.clone(),
                    parallel: indices.len() > 1,
                })
            })
            .collect()
    }

    /// Execute a stage with its failure policy applied.
    async fn execute_stage_with_policy(
        stage: &mut Box<dyn IndexStage>,
        ctx: &mut IndexContext,
    ) -> Result<StageResult> {
        let policy = stage.failure_policy();
        let stage_name = stage.name().to_string();

        match policy {
            FailurePolicy::Fail => {
                // Direct execution, errors propagate
                stage.execute(ctx).await
            }

            FailurePolicy::Skip => {
                // Try once, skip on failure
                match stage.execute(ctx).await {
                    Ok(result) => Ok(result),
                    Err(e) => {
                        warn!("Stage {} failed, skipping: {}", stage_name, e);
                        Ok(StageResult::failure(&stage_name, &e.to_string()))
                    }
                }
            }

            FailurePolicy::Retry(config) => {
                let mut attempts = 0;
                loop {
                    attempts += 1;
                    match stage.execute(ctx).await {
                        Ok(result) => {
                            if attempts > 1 {
                                info!("Stage {} succeeded on attempt {}", stage_name, attempts);
                            }
                            return Ok(result);
                        }
                        Err(e) => {
                            if attempts >= config.max_attempts {
                                warn!(
                                    "Stage {} failed after {} attempts: {}",
                                    stage_name, attempts, e
                                );
                                return Err(e);
                            }
                            let delay = config.delay_for_attempt(attempts - 1);
                            warn!(
                                "Stage {} failed on attempt {}, retrying in {:?}: {}",
                                stage_name, attempts, delay, e
                            );
                            tokio::time::sleep(delay).await;
                        }
                    }
                }
            }
        }
    }

    /// Execute the pipeline.
    ///
    /// Stages are executed in dependency-resolved order.
    /// Failure policies are applied per-stage.
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

        // Compute execution groups for potential parallelization
        let groups = self.compute_execution_groups(&order);
        info!("Execution groups: {} ({} parallelizable)",
            groups.len(),
            groups.iter().filter(|g| g.parallel).count()
        );

        // Create context
        let mut ctx = IndexContext::new(input, options);

        // Execute each group
        for (group_idx, group) in groups.iter().enumerate() {
            if group.parallel {
                info!(
                    "Executing parallel group {} with {} stages: {:?}",
                    group_idx,
                    group.stage_indices.len(),
                    group.stage_indices.iter()
                        .map(|&i| self.stages[i].stage.name())
                        .collect::<Vec<_>>()
                );
            }

            // Execute stages in this group
            // Note: For true parallel execution, stages would need to declare
            // that they don't modify shared context. Currently executed sequentially
            // for safety, but grouped for future optimization.
            for &idx in &group.stage_indices {
                let entry = &mut self.stages[idx];
                let stage_name = entry.stage.name().to_string();
                let policy = entry.stage.failure_policy();

                info!("Executing stage: {} (priority {})", stage_name, entry.priority);

                match Self::execute_stage_with_policy(&mut entry.stage, &mut ctx).await {
                    Ok(result) => {
                        ctx.stage_results.insert(stage_name.clone(), result);
                    }
                    Err(e) => {
                        if policy.allows_continuation() {
                            warn!("Stage {} failed but policy allows continuation: {}", stage_name, e);
                            ctx.stage_results.insert(
                                stage_name.clone(),
                                StageResult::failure(&stage_name, &e.to_string()),
                            );
                        } else {
                            error!("Stage {} failed, stopping pipeline: {}", stage_name, e);
                            return Err(e);
                        }
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

    /// Get execution groups for the current pipeline.
    ///
    /// This is useful for visualizing parallelization opportunities.
    pub fn get_execution_groups(&self) -> Result<Vec<ExecutionGroup>> {
        let order = self.resolve_order()?;
        Ok(self.compute_execution_groups(&order))
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
    use super::super::context::StageResult;

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
