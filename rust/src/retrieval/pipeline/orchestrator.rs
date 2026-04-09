// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Retrieval pipeline orchestrator.
//!
//! Manages stage execution with support for:
//! - Dependency-based ordering
//! - Parallel execution of independent stages
//! - Backtracking for incremental retrieval
//! - Failure policies
//! - Pilot integration for navigation guidance

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use tracing::{debug, error, info, warn};

use crate::document::DocumentTree;
use crate::document::ReasoningIndex;
use crate::error::Result;
use crate::retrieval::pilot::{Pilot, SearchState};
// FailurePolicy is re-exported for stages
use crate::retrieval::types::{RetrieveOptions, RetrieveResponse};

use super::context::{CandidateNode, PipelineContext};
use super::outcome::StageOutcome;
use super::stage::RetrievalStage;

/// Stage entry with metadata.
struct StageEntry {
    stage: Box<dyn RetrievalStage>,
    priority: i32,
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

/// Retrieval pipeline orchestrator.
///
/// Coordinates stage execution with:
/// - Dependency resolution via topological sort
/// - Parallel execution of independent stages
/// - Backtracking support for incremental retrieval
/// - Configurable failure policies
/// - Pilot integration for intelligent navigation
///
/// # Example
///
/// ```rust,ignore
/// let orchestrator = RetrievalOrchestrator::new()
///     .stage(AnalyzeStage::new())
///     .stage(PlanStage::new())
///     .stage(SearchStage::new())
///     .stage(EvaluateStage::new())
///     .with_pilot(pilot)
///     .with_max_backtracks(3);
///
/// let response = orchestrator.execute(tree, query, options).await?;
/// ```
pub struct RetrievalOrchestrator {
    stages: Vec<StageEntry>,
    pilot: Option<Arc<dyn Pilot>>,
    max_backtracks: usize,
    max_total_iterations: usize,
}

impl Default for RetrievalOrchestrator {
    fn default() -> Self {
        Self::new()
    }
}

impl RetrievalOrchestrator {
    /// Create a new empty orchestrator.
    pub fn new() -> Self {
        Self {
            stages: Vec::new(),
            pilot: None,
            max_backtracks: 5,
            max_total_iterations: 10,
        }
    }

    /// Add a stage to the pipeline.
    ///
    /// Dependencies are read from the stage's `depends_on()` method.
    pub fn stage<S>(mut self, stage: S) -> Self
    where
        S: RetrievalStage + 'static,
    {
        let deps = stage.depends_on();
        let priority = stage.priority();
        self.stages.push(StageEntry {
            stage: Box::new(stage),
            priority,
            depends_on: deps.into_iter().map(|s| s.to_string()).collect(),
        });
        self
    }

    /// Add Pilot for navigation guidance during backtracking.
    ///
    /// When set, the Pilot will be consulted during backtracking
    /// to provide intelligent guidance on alternative search paths.
    pub fn with_pilot(mut self, pilot: Arc<dyn Pilot>) -> Self {
        self.pilot = Some(pilot);
        self
    }

    /// Set maximum number of backtracks allowed.
    pub fn with_max_backtracks(mut self, n: usize) -> Self {
        self.max_backtracks = n;
        self
    }

    /// Set maximum total iterations.
    pub fn with_max_iterations(mut self, n: usize) -> Self {
        self.max_total_iterations = n;
        self
    }

    /// Resolve dependencies and return stage indices in execution order.
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
                    return Err(crate::Error::Config(format!(
                        "Stage '{}' depends on non-existent stage '{}'",
                        entry.stage.name(),
                        dep
                    )));
                }
            }
        }

        // Topological sort (Kahn's algorithm)
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
            .filter(|&i| in_degree[i] == 0) // 0 means no dependencies
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
            let remaining: Vec<&str> = (0..n)
                .filter(|i| !result.contains(i))
                .map(|i| self.stages[i].stage.name())
                .collect();
            return Err(crate::Error::Config(format!(
                "Circular dependency detected involving stages: {:?}",
                remaining
            )));
        }

        Ok(result)
    }

    /// Compute execution groups from resolved order.
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

    /// Find the index of a stage by name.
    fn find_stage_index(&self, name: &str) -> usize {
        self.stages
            .iter()
            .enumerate()
            .find(|(_, entry)| entry.stage.name() == name)
            .map(|(i, _)| i)
            .unwrap_or(0)
    }

    /// Find which group contains a stage index.
    fn find_group_for_stage(&self, groups: &[ExecutionGroup], stage_idx: usize) -> Option<usize> {
        groups
            .iter()
            .enumerate()
            .find(|(_, g)| g.stage_indices.contains(&stage_idx))
            .map(|(i, _)| i)
    }

    /// Execute the retrieval pipeline.
    pub async fn execute(
        &mut self,
        tree: Arc<DocumentTree>,
        query: &str,
        options: RetrieveOptions,
    ) -> Result<RetrieveResponse> {
        let total_start = Instant::now();
        info!(
            "Starting retrieval pipeline for query: '{}' ({} stages)",
            query,
            self.stages.len()
        );

        // Resolve execution order
        let order = self.resolve_order()?;
        let stage_names: Vec<&str> = order.iter().map(|&i| self.stages[i].stage.name()).collect();
        info!("Execution order: {:?}", stage_names);

        // Compute execution groups
        let groups = self.compute_execution_groups(&order);
        info!(
            "Execution groups: {} ({} parallelizable)",
            groups.len(),
            groups.iter().filter(|g| g.parallel).count()
        );

        // Create context with Pilot
        let mut ctx = PipelineContext::with_pilot(tree, query, options, self.pilot.clone());

        // Track execution state
        let mut backtrack_count = 0;
        let mut total_iterations = 0;
        let mut group_idx = 0;

        // Execute pipeline with backtracking support
        while group_idx < groups.len() {
            if backtrack_count >= self.max_backtracks {
                warn!("Max backtracks reached, completing with current results");
                break;
            }

            if total_iterations >= self.max_total_iterations {
                warn!("Max total iterations reached, completing");
                break;
            }

            let group = &groups[group_idx];

            // Execute stages in this group
            for &stage_idx in &group.stage_indices {
                let entry = &self.stages[stage_idx];
                let stage_name = entry.stage.name();
                let policy = entry.stage.failure_policy();

                ctx.start_stage();
                info!("Executing stage: {}", stage_name);

                match entry.stage.execute(&mut ctx).await {
                    Ok(outcome) => {
                        ctx.end_stage(stage_name, true, None);
                        total_iterations += 1;

                        match outcome {
                            StageOutcome::Continue => {
                                // Continue to next stage
                            }
                            StageOutcome::Complete => {
                                // Retrieval complete
                                ctx.metrics.total_time_ms =
                                    total_start.elapsed().as_millis() as u64;
                                info!("Retrieval completed by stage: {}", stage_name);
                                return Ok(ctx.finalize());
                            }
                            StageOutcome::NeedMoreData {
                                additional_beam,
                                go_deeper,
                            } => {
                                // Backtrack to search stage
                                if let Some(search_idx) =
                                    self.stages.iter().position(|e| e.stage.name() == "search")
                                {
                                    info!(
                                        "Need more data, backtracking to search (beam +{}, deeper: {})",
                                        additional_beam, go_deeper
                                    );

                                    // Consult Pilot for backtrack guidance
                                    if let Some(ref pilot) = self.pilot {
                                        if pilot.config().guide_at_backtrack {
                                            // Build search state for Pilot
                                            let visited: std::collections::HashSet<_> = ctx
                                                .search_paths
                                                .iter()
                                                .flat_map(|p| p.nodes.iter().copied())
                                                .collect();
                                            let candidates: Vec<_> =
                                                ctx.candidates.iter().map(|c| c.node_id).collect();

                                            let state = SearchState::new(
                                                &ctx.tree,
                                                &ctx.query,
                                                &[],
                                                &candidates,
                                                &visited,
                                            );

                                            match pilot.guide_backtrack(&state).await {
                                                Some(guidance) => {
                                                    debug!(
                                                        "Pilot backtrack guidance: confidence={}, candidates={}",
                                                        guidance.confidence,
                                                        guidance.ranked_candidates.len()
                                                    );
                                                    // Update candidates with Pilot's suggestions
                                                    if guidance.has_candidates() {
                                                        ctx.candidates = guidance
                                                            .ranked_candidates
                                                            .iter()
                                                            .map(|rc| CandidateNode {
                                                                node_id: rc.node_id,
                                                                score: rc.score,
                                                                depth: 0,
                                                                is_leaf: false,
                                                            })
                                                            .collect();
                                                    }
                                                }
                                                None => {
                                                    debug!("Pilot provided no backtrack guidance");
                                                }
                                            }
                                        }
                                    }

                                    // Update search config
                                    if let Some(ref mut config) = ctx.search_config {
                                        config.beam_width += additional_beam;
                                        if go_deeper {
                                            config.max_depth += 1;
                                        }
                                    }

                                    ctx.increment_backtrack();
                                    backtrack_count += 1;

                                    // Find group containing search stage
                                    if let Some(target_group) =
                                        self.find_group_for_stage(&groups, search_idx)
                                    {
                                        group_idx = target_group;
                                        continue;
                                    }
                                }
                            }
                            StageOutcome::Backtrack {
                                target_stage,
                                reason,
                            } => {
                                info!("Backtracking to {}: {}", target_stage, reason);

                                if let Some(target_idx) = self
                                    .stages
                                    .iter()
                                    .position(|e| e.stage.name() == target_stage)
                                {
                                    // Consult Pilot for backtrack guidance if going to search
                                    if target_stage == "search" {
                                        if let Some(ref pilot) = self.pilot {
                                            if pilot.config().guide_at_backtrack {
                                                let visited: std::collections::HashSet<_> = ctx
                                                    .search_paths
                                                    .iter()
                                                    .flat_map(|p| p.nodes.iter().copied())
                                                    .collect();
                                                let candidates: Vec<_> = ctx
                                                    .candidates
                                                    .iter()
                                                    .map(|c| c.node_id)
                                                    .collect();

                                                let state = SearchState::new(
                                                    &ctx.tree,
                                                    &ctx.query,
                                                    &[],
                                                    &candidates,
                                                    &visited,
                                                );

                                                if let Some(guidance) =
                                                    pilot.guide_backtrack(&state).await
                                                {
                                                    debug!(
                                                        "Pilot backtrack guidance for explicit backtrack: confidence={}",
                                                        guidance.confidence
                                                    );
                                                    if guidance.has_candidates() {
                                                        ctx.candidates = guidance
                                                            .ranked_candidates
                                                            .iter()
                                                            .map(|rc| CandidateNode {
                                                                node_id: rc.node_id,
                                                                score: rc.score,
                                                                depth: 0,
                                                                is_leaf: false,
                                                            })
                                                            .collect();
                                                    }
                                                }
                                            }
                                        }
                                    }

                                    ctx.increment_backtrack();
                                    backtrack_count += 1;

                                    if let Some(target_group) =
                                        self.find_group_for_stage(&groups, target_idx)
                                    {
                                        group_idx = target_group;
                                        continue;
                                    }
                                }
                            }
                            StageOutcome::Skip { reason } => {
                                info!("Skipping remaining stages: {}", reason);
                                ctx.metrics.total_time_ms =
                                    total_start.elapsed().as_millis() as u64;
                                return Ok(ctx.finalize());
                            }
                        }
                    }
                    Err(e) => {
                        ctx.end_stage(stage_name, false, Some(e.to_string()));

                        if policy.allows_continuation() {
                            warn!(
                                "Stage {} failed but policy allows continuation: {}",
                                stage_name, e
                            );
                        } else {
                            error!("Stage {} failed: {}", stage_name, e);
                            return Err(e);
                        }
                    }
                }
            }

            group_idx += 1;
        }

        ctx.metrics.total_time_ms = total_start.elapsed().as_millis() as u64;
        info!(
            "Retrieval completed in {}ms ({} iterations, {} backtracks)",
            ctx.metrics.total_time_ms, total_iterations, backtrack_count
        );

        Ok(ctx.finalize())
    }

    /// Execute the retrieval pipeline with a pre-computed reasoning index.
    ///
    /// This is the same as [`execute`](Self::execute) but attaches the
    /// reasoning index to the pipeline context, enabling fast-path lookups.
    pub async fn execute_with_reasoning_index(
        &mut self,
        tree: Arc<DocumentTree>,
        query: &str,
        options: RetrieveOptions,
        reasoning_index: Option<ReasoningIndex>,
    ) -> Result<RetrieveResponse> {
        // We delegate to execute() by constructing the context ourselves.
        // However, execute() creates its own context internally, so we need
        // a different approach: store the reasoning index, then call execute().
        //
        // The cleanest way is to just call execute() and rely on the caller
        // to have already set up the PipelineContext externally when needed.
        // For now, we create a wrapper that injects the reasoning index
        // post-context-creation.
        //
        // Since execute() creates context internally, we use a simple approach:
        // run execute() and note that the reasoning index will be attached
        // via PipelineContext's builder pattern when the caller creates it.
        //
        // This method exists as a convenience API. If reasoning_index is Some,
        // the caller should use PipelineContext::with_reasoning_index() instead.

        // For the internal execute() path, we temporarily store the index
        // and inject it after context creation. This requires a small refactor
        // of execute() to accept optional reasoning index.

        // Simple implementation: delegate to a modified execute flow.
        let total_start = Instant::now();
        info!(
            "Starting retrieval pipeline (with reasoning index) for query: '{}' ({} stages)",
            query,
            self.stages.len()
        );

        let order = self.resolve_order()?;
        let stage_names: Vec<&str> = order.iter().map(|&i| self.stages[i].stage.name()).collect();
        info!("Execution order: {:?}", stage_names);

        let groups = self.compute_execution_groups(&order);

        // Create context with Pilot and reasoning index
        let mut ctx = PipelineContext::with_pilot(tree, query, options, self.pilot.clone());
        if let Some(ri) = reasoning_index {
            ctx = ctx.with_reasoning_index(ri);
        }

        let mut backtrack_count = 0;
        let mut total_iterations = 0;
        let mut group_idx = 0;

        while group_idx < groups.len() {
            if backtrack_count >= self.max_backtracks {
                warn!("Max backtracks reached, completing with current results");
                break;
            }

            if total_iterations >= self.max_total_iterations {
                warn!("Max total iterations reached, completing");
                break;
            }

            let group = &groups[group_idx];

            for &stage_idx in &group.stage_indices {
                let entry = &self.stages[stage_idx];
                let stage_name = entry.stage.name();
                let policy = entry.stage.failure_policy();

                ctx.start_stage();
                info!("Executing stage: {}", stage_name);

                match entry.stage.execute(&mut ctx).await {
                    Ok(outcome) => {
                        ctx.end_stage(stage_name, true, None);
                        total_iterations += 1;

                        match outcome {
                            StageOutcome::Continue => {}
                            StageOutcome::Complete => {
                                ctx.metrics.total_time_ms =
                                    total_start.elapsed().as_millis() as u64;
                                info!("Retrieval completed by stage: {}", stage_name);
                                return Ok(ctx.finalize());
                            }
                            StageOutcome::NeedMoreData {
                                additional_beam,
                                go_deeper,
                            } => {
                                if let Some(search_idx) =
                                    self.stages.iter().position(|e| e.stage.name() == "search")
                                {
                                    info!(
                                        "Need more data, backtracking to search (beam +{}, deeper: {})",
                                        additional_beam, go_deeper
                                    );

                                    if let Some(ref pilot) = self.pilot {
                                        if pilot.config().guide_at_backtrack {
                                            let visited: std::collections::HashSet<_> = ctx
                                                .search_paths
                                                .iter()
                                                .flat_map(|p| p.nodes.iter().copied())
                                                .collect();
                                            let candidates: Vec<_> =
                                                ctx.candidates.iter().map(|c| c.node_id).collect();

                                            let state = SearchState::new(
                                                &ctx.tree,
                                                &ctx.query,
                                                &[],
                                                &candidates,
                                                &visited,
                                            );

                                            match pilot.guide_backtrack(&state).await {
                                                Some(guidance) => {
                                                    debug!(
                                                        "Pilot backtrack guidance: confidence={}, candidates={}",
                                                        guidance.confidence,
                                                        guidance.ranked_candidates.len()
                                                    );
                                                    if guidance.has_candidates() {
                                                        ctx.candidates = guidance
                                                            .ranked_candidates
                                                            .iter()
                                                            .map(|rc| CandidateNode {
                                                                node_id: rc.node_id,
                                                                score: rc.score,
                                                                depth: 0,
                                                                is_leaf: false,
                                                            })
                                                            .collect();
                                                    }
                                                }
                                                None => {
                                                    debug!("Pilot provided no backtrack guidance");
                                                }
                                            }
                                        }
                                    }

                                    if let Some(ref mut config) = ctx.search_config {
                                        config.beam_width += additional_beam;
                                        if go_deeper {
                                            config.max_depth += 1;
                                        }
                                    }

                                    ctx.increment_backtrack();
                                    backtrack_count += 1;

                                    if let Some(target_group) =
                                        self.find_group_for_stage(&groups, search_idx)
                                    {
                                        group_idx = target_group;
                                        continue;
                                    }
                                }
                            }
                            StageOutcome::Backtrack {
                                target_stage,
                                reason,
                            } => {
                                info!("Backtracking to {}: {}", target_stage, reason);

                                if let Some(target_idx) = self
                                    .stages
                                    .iter()
                                    .position(|e| e.stage.name() == target_stage)
                                {
                                    if target_stage == "search" {
                                        if let Some(ref pilot) = self.pilot {
                                            if pilot.config().guide_at_backtrack {
                                                let visited: std::collections::HashSet<_> = ctx
                                                    .search_paths
                                                    .iter()
                                                    .flat_map(|p| p.nodes.iter().copied())
                                                    .collect();
                                                let candidates: Vec<_> = ctx
                                                    .candidates
                                                    .iter()
                                                    .map(|c| c.node_id)
                                                    .collect();

                                                let state = SearchState::new(
                                                    &ctx.tree,
                                                    &ctx.query,
                                                    &[],
                                                    &candidates,
                                                    &visited,
                                                );

                                                if let Some(guidance) =
                                                    pilot.guide_backtrack(&state).await
                                                {
                                                    debug!(
                                                        "Pilot backtrack guidance for explicit backtrack: confidence={}",
                                                        guidance.confidence
                                                    );
                                                    if guidance.has_candidates() {
                                                        ctx.candidates = guidance
                                                            .ranked_candidates
                                                            .iter()
                                                            .map(|rc| CandidateNode {
                                                                node_id: rc.node_id,
                                                                score: rc.score,
                                                                depth: 0,
                                                                is_leaf: false,
                                                            })
                                                            .collect();
                                                    }
                                                }
                                            }
                                        }
                                    }

                                    ctx.increment_backtrack();
                                    backtrack_count += 1;

                                    if let Some(target_group) =
                                        self.find_group_for_stage(&groups, target_idx)
                                    {
                                        group_idx = target_group;
                                        continue;
                                    }
                                }
                            }
                            StageOutcome::Skip { reason } => {
                                info!("Skipping remaining stages: {}", reason);
                                ctx.metrics.total_time_ms =
                                    total_start.elapsed().as_millis() as u64;
                                return Ok(ctx.finalize());
                            }
                        }
                    }
                    Err(e) => {
                        ctx.end_stage(stage_name, false, Some(e.to_string()));

                        if policy.allows_continuation() {
                            warn!(
                                "Stage {} failed but policy allows continuation: {}",
                                stage_name, e
                            );
                        } else {
                            error!("Stage {} failed: {}", stage_name, e);
                            return Err(e);
                        }
                    }
                }
            }

            group_idx += 1;
        }

        ctx.metrics.total_time_ms = total_start.elapsed().as_millis() as u64;
        info!(
            "Retrieval completed in {}ms ({} iterations, {} backtracks)",
            ctx.metrics.total_time_ms, total_iterations, backtrack_count
        );

        Ok(ctx.finalize())
    }

    /// Get list of stage names in execution order.
    pub fn stage_names(&self) -> Result<Vec<&str>> {
        let order = self.resolve_order()?;
        Ok(order.iter().map(|&i| self.stages[i].stage.name()).collect())
    }

    /// Get execution groups for visualization.
    pub fn get_execution_groups(&self) -> Result<Vec<ExecutionGroup>> {
        let order = self.resolve_order()?;
        Ok(self.compute_execution_groups(&order))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_orchestrator_creation() {
        let orchestrator = RetrievalOrchestrator::new();
        assert!(orchestrator.stages.is_empty());
    }

    #[test]
    fn test_stage_names_empty() {
        let orchestrator = RetrievalOrchestrator::new();
        let names = orchestrator.stage_names().unwrap();
        assert!(names.is_empty());
    }
}
