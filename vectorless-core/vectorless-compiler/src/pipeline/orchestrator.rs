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
use tracing::{debug, error, info, warn};

use vectorless_error::Result;

use super::super::PipelineOptions;
use super::super::stages::CompileStage;
use super::checkpoint::{CheckpointContextData, CheckpointManager, PipelineCheckpoint};
use super::context::{CompileContext, CompilerInput, CompileResult, StageResult};
use super::policy::FailurePolicy;

/// Stage entry with metadata for orchestration.
struct StageEntry {
    /// The stage implementation.
    stage: Box<dyn CompileStage>,
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
    /// Shared LLM client injected into pipeline context.
    llm_client: Option<vectorless_llm::LlmClient>,
}

impl Default for PipelineOrchestrator {
    fn default() -> Self {
        Self::new()
    }
}

impl PipelineOrchestrator {
    /// Create a new empty orchestrator.
    pub fn new() -> Self {
        Self {
            stages: Vec::new(),
            llm_client: None,
        }
    }

    /// Set the shared LLM client (injected into pipeline context).
    pub fn with_llm_client(mut self, client: vectorless_llm::LlmClient) -> Self {
        self.llm_client = Some(client);
        self
    }

    /// Add a stage with default priority (100).
    ///
    /// Dependencies are automatically read from the stage's `depends_on()` method.
    pub fn stage<S>(mut self, stage: S) -> Self
    where
        S: CompileStage + 'static,
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
        S: CompileStage + 'static,
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
        S: CompileStage + 'static,
    {
        let trait_deps = stage.depends_on();
        let mut all_deps: Vec<String> = trait_deps.into_iter().map(|s| s.to_string()).collect();

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
                    return Err(vectorless_error::Error::Config(format!(
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
        let mut ready: Vec<usize> = (0..n).filter(|&i| in_degree[i] == 0).collect();
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
            let remaining: Vec<&str> = (0..n)
                .filter(|i| !result.contains(i))
                .map(|i| self.stages[i].stage.name())
                .collect();
            return Err(vectorless_error::Error::Config(format!(
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
        stage: &mut Box<dyn CompileStage>,
        ctx: &mut CompileContext,
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

    /// Handle the result of a stage execution (shared between sequential and parallel paths).
    fn handle_stage_result(
        result: Result<StageResult>,
        stage_name: &str,
        policy: &FailurePolicy,
        ctx: &mut CompileContext,
    ) -> Result<()> {
        match result {
            Ok(result) => {
                ctx.stage_results.insert(stage_name.to_string(), result);
                Ok(())
            }
            Err(e) => {
                if policy.allows_continuation() {
                    warn!(
                        "Stage {} failed but policy allows continuation: {}",
                        stage_name, e
                    );
                    ctx.stage_results.insert(
                        stage_name.to_string(),
                        StageResult::failure(stage_name, &e.to_string()),
                    );
                    Ok(())
                } else {
                    error!("Stage {} failed, stopping pipeline: {}", stage_name, e);
                    Err(e)
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
        input: CompilerInput,
        options: PipelineOptions,
    ) -> Result<CompileResult> {
        let total_start = Instant::now();
        info!(
            "Starting orchestrated pipeline with {} stages",
            self.stages.len()
        );

        // Resolve execution order
        let order = self.resolve_order()?;
        let stage_names: Vec<&str> = order.iter().map(|&i| self.stages[i].stage.name()).collect();
        info!("[pipeline] Execution order: {:?}", stage_names);

        // Compute execution groups for potential parallelization
        let groups = self.compute_execution_groups(&order);
        let parallel_count = groups.iter().filter(|g| g.parallel).count();
        if parallel_count > 0 {
            info!(
                "[pipeline] {} execution groups ({} parallelizable)",
                groups.len(),
                parallel_count
            );
        } else {
            debug!(
                "[pipeline] {} execution groups (all sequential)",
                groups.len()
            );
        }

        // Create context
        let mut opts = options;
        let existing_tree = opts.existing_tree.take();
        let mut ctx = CompileContext::new(input, opts);
        // Inject shared LLM client into context for stages that need it (e.g. ReasoningCompileStage)
        if let Some(client) = self.llm_client.take() {
            ctx = ctx.with_llm_client(client);
        }
        if let Some(tree) = existing_tree {
            ctx = ctx.with_existing_tree(tree);
        }

        // Try to resume from checkpoint
        if let Some(ref checkpoint_dir) = ctx.options.checkpoint_dir {
            let manager = CheckpointManager::new(checkpoint_dir);
            if let Some(checkpoint) = manager.load(&ctx.doc_id) {
                if CheckpointManager::is_valid_for_resume(
                    &checkpoint,
                    &ctx.source_hash,
                    ctx.options.processing_version,
                    &ctx.options.logic_fingerprint().to_string(),
                ) {
                    info!(
                        "Resuming from checkpoint: {} stages already completed",
                        checkpoint.completed_stages.len()
                    );
                    // Restore context data from checkpoint
                    ctx.raw_nodes = checkpoint.context_data.raw_nodes;
                    if let Some(tree) = checkpoint.context_data.tree {
                        ctx.tree = Some(tree);
                    }
                    ctx.metrics = checkpoint.context_data.metrics;
                    ctx.page_count = checkpoint.context_data.page_count;
                    ctx.line_count = checkpoint.context_data.line_count;
                    ctx.description = checkpoint.context_data.description;
                    // Mark completed stages as done
                    for stage_name in &checkpoint.completed_stages {
                        ctx.stage_results
                            .insert(stage_name.clone(), StageResult::success(stage_name));
                    }
                } else {
                    info!("Checkpoint exists but invalid, starting fresh");
                }
            }
        }

        // Execute each group
        for (group_idx, group) in groups.iter().enumerate() {
            if group.parallel {
                let names: Vec<&str> = group
                    .stage_indices
                    .iter()
                    .map(|&i| self.stages[i].stage.name())
                    .collect();
                info!("[pipeline] Parallel group {}: {:?}", group_idx, names);
            }

            if group.parallel && !group.stage_indices.is_empty() {
                // Check if all stages in this group are already completed (from checkpoint)
                let all_completed = group.stage_indices.iter().all(|&idx| {
                    let name = self.stages[idx].stage.name();
                    ctx.stage_results.contains_key(name)
                });
                if all_completed {
                    let names: Vec<&str> = group
                        .stage_indices
                        .iter()
                        .map(|&i| self.stages[i].stage.name())
                        .collect();
                    info!("[pipeline] Skipping completed parallel group: {:?}", names);
                    continue;
                }

                // === N-stage parallel execution ===
                //
                // At most one stage may write_tree — it gets the main ctx.
                // All other stages get cloned contexts with tree snapshots.
                // All stages run concurrently via futures::future::join_all.
                // After all complete, outputs are merged back by AccessPattern.

                // Identify the tree writer (if any)
                let tree_writer_idx: Option<usize> = group
                    .stage_indices
                    .iter()
                    .find(|&&idx| self.stages[idx].stage.access_pattern().writes_tree)
                    .copied();

                // For each stage, prepare (stage, context) pair.
                // Swap out stages from self.stages to get owned Box<dyn CompileStage>.
                let mut entries: Vec<ParallelEntry> = Vec::with_capacity(group.stage_indices.len());

                for &idx in &group.stage_indices {
                    let stage = std::mem::replace(&mut self.stages[idx].stage, Box::new(NopStage));
                    let name = stage.name().to_string();
                    let policy = stage.failure_policy();
                    let access = stage.access_pattern();

                    let stage_ctx = if Some(idx) == tree_writer_idx {
                        // Tree writer gets a placeholder; we'll use &mut ctx directly
                        None
                    } else {
                        // Reader gets a cloned context
                        let mut clone =
                            CompileContext::new(CompilerInput::content(""), ctx.options.clone());
                        clone.tree = ctx.tree.clone();
                        clone.existing_tree = ctx.existing_tree.clone();
                        clone.doc_id = ctx.doc_id.clone();
                        clone.name = ctx.name.clone();
                        clone.format = ctx.format;
                        clone.source_path = ctx.source_path.clone();
                        if let Some(ref llm) = ctx.llm_client {
                            clone.llm_client = Some(llm.clone());
                        }
                        Some(clone)
                    };

                    entries.push(ParallelEntry {
                        idx,
                        stage,
                        ctx: stage_ctx,
                        name,
                        policy,
                        access,
                    });
                }

                let parallel_names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
                info!("[pipeline] Executing in parallel: {:?}", parallel_names);

                // Split into writer and readers
                let mut writer_entry: Option<ParallelEntry> = None;
                let mut reader_entries: Vec<ParallelEntry> = Vec::new();
                for entry in entries {
                    if entry.ctx.is_none() {
                        writer_entry = Some(entry);
                    } else {
                        reader_entries.push(entry);
                    }
                }

                // Execute writer on main ctx concurrently with readers.
                // Move each reader's stage+ctx into an owned async block.
                // All futures are !Send (Box<dyn CompileStage>), but join_all
                // works fine on the same thread.

                let reader_futs: Vec<
                    std::pin::Pin<
                        Box<
                            dyn std::future::Future<
                                    Output = (
                                        ParallelEntry,
                                        std::result::Result<StageResult, vectorless_error::Error>,
                                    ),
                                > + Send,
                        >,
                    >,
                > = reader_entries
                    .into_iter()
                    .map(|mut entry| {
                        Box::pin(async move {
                            let res = Self::execute_stage_with_policy(
                                &mut entry.stage,
                                entry.ctx.as_mut().unwrap(),
                            )
                            .await;
                            (entry, res)
                        })
                            as std::pin::Pin<Box<dyn std::future::Future<Output = _> + Send>>
                    })
                    .collect();

                // If there's a tree writer, run it concurrently with readers.
                // If no tree writer (all readers), just run readers.
                if let Some(mut we) = writer_entry {
                    // Run writer + readers concurrently.
                    // The writer borrows &mut ctx; readers use their own cloned ctxs.
                    let (writer_res, completed_readers) = tokio::join!(
                        Self::execute_stage_with_policy(&mut we.stage, &mut ctx),
                        futures::future::join_all(reader_futs),
                    );

                    // Put writer stage back and handle result
                    self.stages[we.idx].stage = we.stage;
                    Self::handle_stage_result(writer_res, &we.name, &we.policy, &mut ctx)?;

                    // Process reader results
                    for (re, reader_res) in completed_readers {
                        Self::merge_reader_outputs(&mut ctx, &re);
                        self.stages[re.idx].stage = re.stage;
                        Self::handle_stage_result(reader_res, &re.name, &re.policy, &mut ctx)?;
                    }
                } else {
                    // All readers, no writer
                    let completed_readers = futures::future::join_all(reader_futs).await;
                    for (re, reader_res) in completed_readers {
                        Self::merge_reader_outputs(&mut ctx, &re);
                        self.stages[re.idx].stage = re.stage;
                        Self::handle_stage_result(reader_res, &re.name, &re.policy, &mut ctx)?;
                    }
                }
            } else {
                // === Sequential execution (single stage or non-parallel group) ===
                for &idx in &group.stage_indices {
                    let entry = &mut self.stages[idx];
                    let stage_name = entry.stage.name().to_string();

                    // Skip stages already completed (from checkpoint resume)
                    if ctx.stage_results.contains_key(&stage_name) {
                        info!("Skipping already completed stage: {}", stage_name);
                        continue;
                    }

                    let policy = entry.stage.failure_policy();

                    info!(
                        "Executing stage: {} (priority {})",
                        stage_name, entry.priority
                    );

                    match Self::execute_stage_with_policy(&mut entry.stage, &mut ctx).await {
                        Ok(result) => {
                            ctx.stage_results.insert(stage_name.clone(), result);
                        }
                        Err(e) => {
                            if policy.allows_continuation() {
                                warn!(
                                    "Stage {} failed but policy allows continuation: {}",
                                    stage_name, e
                                );
                                ctx.stage_results.insert(
                                    stage_name.clone(),
                                    StageResult::failure(&stage_name, &e.to_string()),
                                );
                            } else {
                                error!("Stage {} failed, stopping pipeline: {}", stage_name, e);
                                // Save checkpoint before returning error
                                Self::save_checkpoint(&ctx);
                                return Err(e);
                            }
                        }
                    }
                }
            }

            // Save checkpoint after each group completes
            Self::save_checkpoint(&ctx);
        }

        let total_duration = total_start.elapsed().as_millis() as u64;
        info!(
            "[pipeline] Complete: {} stages in {}ms for '{}'",
            ctx.stage_results.len(),
            total_duration,
            ctx.name,
        );

        // Clear checkpoint on successful completion
        if let Some(ref checkpoint_dir) = ctx.options.checkpoint_dir {
            let manager = CheckpointManager::new(checkpoint_dir);
            if let Err(e) = manager.clear(&ctx.doc_id) {
                warn!("Failed to clear checkpoint for {}: {}", ctx.doc_id, e);
            }
        }

        // Finalize result
        Ok(ctx.finalize())
    }

    /// Merge a reader stage's outputs back into the main context.
    ///
    /// Reads the reader's AccessPattern to know which fields to copy,
    /// and merges additive metrics (LLM calls, tokens, etc.).
    fn merge_reader_outputs(ctx: &mut CompileContext, reader: &ParallelEntry) {
        if reader.access.writes_reasoning_index {
            if let Some(ref rctx) = reader.ctx {
                ctx.reasoning_index = rctx.reasoning_index.clone();
            }
        }
        if reader.access.writes_navigation_index {
            if let Some(ref rctx) = reader.ctx {
                ctx.navigation_index = rctx.navigation_index.clone();
            }
        }
        if reader.access.writes_description {
            if let Some(ref rctx) = reader.ctx {
                ctx.description = rctx.description.clone();
            }
        }
        // Merge additive metrics from reader
        if let Some(ref rctx) = reader.ctx {
            ctx.metrics.llm_calls += rctx.metrics.llm_calls;
            ctx.metrics.summaries_generated += rctx.metrics.summaries_generated;
            ctx.metrics.total_tokens_generated += rctx.metrics.total_tokens_generated;
            ctx.metrics.nodes_processed += rctx.metrics.nodes_processed;
            ctx.metrics.nodes_merged += rctx.metrics.nodes_merged;
            ctx.metrics.nodes_skipped += rctx.metrics.nodes_skipped;
            if rctx.metrics.reasoning_index_time_ms > 0 {
                ctx.metrics.record_reasoning_index(
                    rctx.metrics.reasoning_index_time_ms,
                    rctx.metrics.topics_indexed,
                    rctx.metrics.keywords_indexed,
                );
            }
            if rctx.metrics.optimize_time_ms > 0 {
                ctx.metrics.record_optimize(rctx.metrics.optimize_time_ms);
            }
            if rctx.metrics.navigation_index_time_ms > 0 {
                ctx.metrics.record_navigation_index(
                    rctx.metrics.navigation_index_time_ms,
                    rctx.metrics.nav_entries_indexed,
                    rctx.metrics.child_routes_indexed,
                );
            }
            if rctx.metrics.enhance_time_ms > 0 {
                ctx.metrics.record_enhance(rctx.metrics.enhance_time_ms);
            }
            if rctx.metrics.enrich_time_ms > 0 {
                ctx.metrics.record_enrich(rctx.metrics.enrich_time_ms);
            }
        }
    }

    /// Save a checkpoint of the current pipeline state.
    fn save_checkpoint(ctx: &CompileContext) {
        let checkpoint_dir = match ctx.options.checkpoint_dir {
            Some(ref dir) => dir.clone(),
            None => return,
        };

        let completed_stages: Vec<String> = ctx.stage_results.keys().cloned().collect();
        let checkpoint = PipelineCheckpoint {
            doc_id: ctx.doc_id.clone(),
            source_hash: ctx.source_hash.clone(),
            processing_version: ctx.options.processing_version,
            config_fingerprint: ctx.options.logic_fingerprint().to_string(),
            completed_stages,
            context_data: CheckpointContextData {
                raw_nodes: ctx.raw_nodes.clone(),
                tree: ctx.tree.clone(),
                metrics: ctx.metrics.clone(),
                page_count: ctx.page_count,
                line_count: ctx.line_count,
                description: ctx.description.clone(),
            },
            timestamp: chrono::Utc::now(),
        };

        let manager = CheckpointManager::new(checkpoint_dir);
        if let Err(e) = manager.save(&ctx.doc_id, &checkpoint) {
            warn!("Failed to save checkpoint for {}: {}", ctx.doc_id, e);
        }
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

/// Placeholder stage used during parallel execution when the real stage
/// is temporarily swapped out via `std::mem::replace`.
struct NopStage;

#[async_trait::async_trait]
impl CompileStage for NopStage {
    fn name(&self) -> &'static str {
        "_nop"
    }

    async fn execute(&mut self, _ctx: &mut CompileContext) -> Result<StageResult> {
        Ok(StageResult::success("_nop"))
    }
}

/// Owned entry for parallel stage execution.
///
/// Each stage in a parallel group is swapped out from the orchestrator's
/// stages vec into this struct, along with its own cloned context.
/// After execution, the stage is swapped back and outputs are merged.
struct ParallelEntry {
    /// Index into orchestrator's stages vec (for swapping back).
    idx: usize,
    /// The owned stage implementation.
    stage: Box<dyn CompileStage>,
    /// Cloned context for reader stages; None for the tree writer
    /// (which uses the main ctx directly).
    ctx: Option<CompileContext>,
    /// Stage name (captured before swap).
    name: String,
    /// Failure policy (captured before swap).
    policy: FailurePolicy,
    /// Access pattern (captured before swap).
    access: crate::stages::AccessPattern,
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
    use super::super::context::StageResult;
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
        let orchestrator =
            PipelineOrchestrator::new().stage_with_deps(MockStage::new("a"), 10, &["nonexistent"]);

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
            Self {
                name: name.to_string(),
            }
        }
    }

    #[async_trait::async_trait]
    impl CompileStage for MockStage {
        fn name(&self) -> &str {
            &self.name
        }

        async fn execute(&mut self, _ctx: &mut CompileContext) -> Result<StageResult> {
            Ok(StageResult::success(&self.name))
        }
    }
}
