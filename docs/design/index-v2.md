# Index Pipeline v2 Design

## Overview

This document describes the new Index Pipeline design for Vectorless.

## Design Goals

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         Index Pipeline Design Goals                          │
├─────────────────────────────────────────────────────────────────────────────┤
│  1. Modular     — Each stage is independent and testable                     │
│  2. Configurable — Support different processing strategies                   │
│  3. Extensible   — Easy to add new stages                                   │
│  4. Incremental  — Efficient re-indexing on document changes                │
│  5. Observable   — Metrics for each stage                                   │
└─────────────────────────────────────────────────────────────────────────────┘
```

## Architecture

### Pipeline Overview

```
┌─────────────┐    ┌─────────────┐    ┌─────────────┐    ┌─────────────┐
│   Input     │───►│   Parse     │───►│   Build     │───►│  Enhance    │
│  (File/Text)│    │  (Document) │    │   (Tree)    │    │  (LLM Boost)│
└─────────────┘    └─────────────┘    └─────────────┘    └─────────────┘
                                                                   │
                                                                   ▼
┌─────────────┐    ┌─────────────┐    ┌─────────────┐    ┌─────────────┐
│   Output    │◄───│   Persist   │◄───│   Enrich    │◄───│   Optimize  │
│  (Indexed)  │    │  (Storage)  │    │  (Metadata) │    │  (Tree)     │
└─────────────┘    └─────────────┘    └─────────────┘    └─────────────┘
```

### Directory Structure

```
src/core/
├── mod.rs
├── error.rs
├── tree/                           # Existing tree structure
│   ├── mod.rs
│   ├── node.rs
│   ├── tree.rs
│   └── toc.rs
│
├── retriever/                      # Existing retriever module
│   └── ...
│
└── index/                          # NEW: Index Pipeline
    ├── mod.rs
    │
    ├── pipeline/
    │   ├── mod.rs
    │   ├── executor.rs             # Pipeline executor
    │   ├── context.rs              # IndexContext
    │   ├── stage.rs                # IndexStage trait
    │   └── metrics.rs              # Performance metrics
    │
    ├── stages/
    │   ├── mod.rs
    │   ├── parse.rs                # Parse stage
    │   ├── build.rs                # Build stage
    │   ├── enhance.rs              # LLM enhance stage
    │   ├── enrich.rs               # Metadata enrich stage
    │   ├── optimize.rs             # Tree optimize stage
    │   └── persist.rs              # Persist stage
    │
    ├── summary/
    │   ├── mod.rs
    │   ├── strategy.rs             # SummaryStrategy trait
    │   ├── full.rs                 # Full strategy
    │   ├── selective.rs            # Selective strategy
    │   └── lazy.rs                 # Lazy strategy
    │
    └── incremental/
        ├── mod.rs
        ├── detector.rs             # Change detection
        └── updater.rs              # Partial update
```

## Stage Details

### Stage 1: Parse

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                              Parse Stage                                     │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  Input:  File path or raw text                                              │
│  Output: Vec<RawNode>                                                       │
│                                                                              │
│  ┌────────────────────────────────────────────────────────────────────┐     │
│  │                        ParserRegistry                               │     │
│  │                                                                     │     │
│  │   ┌──────────┐   ┌──────────┐   ┌──────────┐   ┌──────────┐       │     │
│  │   │ Markdown │   │   PDF    │   │  DOCX    │   │   HTML   │       │     │
│  │   │ Parser   │   │  Parser  │   │  Parser  │   │  Parser  │       │     │
│  │   └──────────┘   └──────────┘   └──────────┘   └──────────┘       │     │
│  │                                                                     │     │
│  └────────────────────────────────────────────────────────────────────┘     │
│                                                                              │
│  RawNode fields:                                                             │
│  • title, content, level                                                     │
│  • line_start, line_end                                                      │
│  • page (for PDF)                                                            │
│  • token_count (estimated)                                                   │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Stage 2: Build

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                              Build Stage                                     │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  Input:  Vec<RawNode>                                                       │
│  Output: VectorlessTree                                                     │
│                                                                              │
│  Step 1: Token Calculation (recursive)                                       │
│  ┌────────────────────────────────────────────────────────────────────┐     │
│  │  total_token_count = own_tokens + Σ(children's total_token_count)  │     │
│  └────────────────────────────────────────────────────────────────────┘     │
│                                                                              │
│  Step 2: Thinning (optional)                                                 │
│  ┌────────────────────────────────────────────────────────────────────┐     │
│  │  if total_tokens < threshold:                                       │     │
│  │      merge into parent                                              │     │
│  │                                                                     │     │
│  │  Rule: Ensure each parent keeps at least one child                  │     │
│  └────────────────────────────────────────────────────────────────────┘     │
│                                                                              │
│  Step 3: Hierarchy Construction                                              │
│  ┌────────────────────────────────────────────────────────────────────┐     │
│  │  level_stack: Track most recent parent at each level               │     │
│  │                                                                     │     │
│  │  for node in raw_nodes:                                             │     │
│  │      parent = find_parent(level_stack, node.level)                  │     │
│  │      tree.add_child(parent, node)                                   │     │
│  │      update_level_stack(level_stack, node)                          │     │
│  └────────────────────────────────────────────────────────────────────┘     │
│                                                                              │
│  Step 4: ID Assignment                                                       │
│  ┌────────────────────────────────────────────────────────────────────┐     │
│  │  DFS traversal, assign incremental IDs: "0001", "0002", ...         │     │
│  └────────────────────────────────────────────────────────────────────┘     │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Stage 3: Enhance

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                             Enhance Stage                                    │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  Input:  VectorlessTree                                                     │
│  Output: VectorlessTree (with summaries)                                    │
│                                                                              │
│  Strategy Selection:                                                         │
│  ┌────────────────────────────────────────────────────────────────────┐     │
│  │                                                                     │     │
│  │   ┌─────────────┐   ┌─────────────┐   ┌─────────────┐            │     │
│  │   │    Full     │   │  Selective  │   │    Lazy     │            │     │
│  │   │  (All nodes)│   │ (Branch only)│  │  (On-demand)│            │     │
│  │   └─────────────┘   └─────────────┘   └─────────────┘            │     │
│  │                                                                     │     │
│  │   • Generate all    • Branch nodes only  • Generate at query      │     │
│  │   • Slow indexing   • Balanced          • Fastest indexing        │     │
│  │   • Fast query      • Recommended        • Slow first query       │     │
│  │                                                                     │     │
│  └────────────────────────────────────────────────────────────────────┘     │
│                                                                              │
│  Lazy Summary Persistence:                                                   │
│  ┌────────────────────────────────────────────────────────────────────┐     │
│  │  When summary is generated on-demand:                               │     │
│  │  1. Cache in memory (LRU)                                           │     │
│  │  2. Optionally persist to disk (configurable)                       │     │
│  │  3. Update tree metadata                                            │     │
│  └────────────────────────────────────────────────────────────────────┘     │
│                                                                              │
│  Concurrency Control:                                                        │
│  ┌────────────────────────────────────────────────────────────────────┐     │
│  │  • Semaphore: Limit concurrent LLM calls (default: 10)              │     │
│  │  • Rate Limit: RPM control (default: 500)                           │     │
│  │  • Retry: Exponential backoff on failure                            │     │
│  │  • Fallback: Model degradation on repeated failures                 │     │
│  └────────────────────────────────────────────────────────────────────┘     │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Stage 4: Enrich

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                             Enrich Stage                                     │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  Input:  VectorlessTree                                                     │
│  Output: VectorlessTree (enriched metadata)                                 │
│                                                                              │
│  ┌────────────────────────────────────────────────────────────────────┐     │
│  │                                                                     │     │
│  │  1. ToC View Generation                                             │     │
│  │     • Hierarchical directory view                                   │     │
│  │     • Used for LLM navigation context                               │     │
│  │                                                                     │     │
│  │  2. Page Range Calculation                                          │     │
│  │     • Derive parent page range from children                        │     │
│  │                                                                     │     │
│  │  3. Token Statistics                                                │     │
│  │     • Subtree token totals                                          │     │
│  │                                                                     │     │
│  │  4. Document Description (optional)                                 │     │
│  │     • Use root summary or LLM generation                            │     │
│  │                                                                     │     │
│  └────────────────────────────────────────────────────────────────────┘     │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Stage 5: Optimize

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                            Optimize Stage                                    │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  Input:  VectorlessTree                                                     │
│  Output: VectorlessTree (optimized)                                         │
│                                                                              │
│  Optimization Strategies:                                                    │
│  ┌────────────────────────────────────────────────────────────────────┐     │
│  │                                                                     │     │
│  │  1. Depth Optimization                                             │     │
│  │     if tree_depth > max_depth:                                      │     │
│  │         flatten_deep_nodes()                                        │     │
│  │                                                                     │     │
│  │  2. Width Optimization                                             │     │
│  │     if children_count > max_children:                               │     │
│  │         group_similar_children()                                    │     │
│  │                                                                     │     │
│  │  3. Leaf Merging                                                   │     │
│  │     merge_adjacent_small_leaves(min_tokens: 100)                    │     │
│  │                                                                     │     │
│  │  4. Empty Node Cleanup                                             │     │
│  │     remove_empty_intermediate_nodes()                               │     │
│  │                                                                     │     │
│  └────────────────────────────────────────────────────────────────────┘     │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Stage 6: Persist

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                             Persist Stage                                    │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  Input:  VectorlessTree                                                     │
│  Output: PersistedDocument                                                  │
│                                                                              │
│  Workspace Structure:                                                        │
│  ┌────────────────────────────────────────────────────────────────────┐     │
│  │                                                                     │     │
│  │  workspace/                                                         │     │
│  │  ├── _meta.json           # Index metadata                         │     │
│  │  │   {                                                             │     │
│  │  │     "version": "2.0",                                           │     │
│  │  │     "documents": [...]                                          │     │
│  │  │   }                                                             │     │
│  │  │                                                                 │     │
│  │  ├── {doc_id}.json        # Complete document tree                 │     │
│  │  │   {                                                             │     │
│  │  │     "meta": {...},                                              │     │
│  │  │     "tree": {...},                                              │     │
│  │  │     "pages": [...]                                              │     │
│  │  │   }                                                             │     │
│  │  │                                                                 │     │
│  │  └── cache/               # Optional cache                         │     │
│  │      └── summaries/       # Lazy-generated summaries               │     │
│  │          └── {doc_id}_{node_id}.txt                                │     │
│  │                                                                     │     │
│  └────────────────────────────────────────────────────────────────────┘     │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

## Core Types

### IndexContext

```rust
/// Index context passed between stages
pub struct IndexContext {
    /// Document ID
    pub doc_id: String,
    /// Source file path
    pub source_path: Option<PathBuf>,
    /// Document format
    pub format: DocumentFormat,
    /// Parsed raw nodes
    pub raw_nodes: Vec<RawNode>,
    /// Built tree
    pub tree: Option<VectorlessTree>,
    /// Index options
    pub options: IndexOptions,
    /// Stage execution results
    pub stage_results: HashMap<String, StageResult>,
    /// Performance metrics
    pub metrics: IndexMetrics,
}
```

### IndexStage Trait

```rust
/// Index pipeline stage
pub trait IndexStage: Send + Sync {
    /// Stage name
    fn name(&self) -> &str;

    /// Execute stage
    async fn execute(&self, ctx: &mut IndexContext) -> Result<StageResult>;

    /// Whether this stage is optional (can be skipped on failure)
    fn is_optional(&self) -> bool {
        false
    }
}
```

### SummaryStrategy

```rust
/// Summary generation strategy
pub enum SummaryStrategy {
    /// No summary generation
    None,

    /// Generate for all nodes
    Full,

    /// Generate selectively (branch nodes, min tokens threshold)
    Selective {
        min_tokens: usize,
        branch_only: bool,
    },

    /// Generate on-demand at query time
    Lazy {
        persist: bool,  // Whether to persist generated summaries
    },
}
```

### IndexOptions

```rust
/// Index options (v2)
pub struct IndexOptions {
    /// Index mode
    pub mode: IndexMode,

    /// Whether to generate node IDs
    pub generate_ids: bool,

    /// Summary generation strategy
    pub summary_strategy: SummaryStrategy,

    /// Thinning configuration
    pub thinning: ThinningConfig,

    /// Optimization configuration
    pub optimization: OptimizationConfig,

    /// Whether to generate document description
    pub generate_description: bool,

    /// Concurrency configuration
    pub concurrency: ConcurrencyConfig,
}

impl Default for IndexOptions {
    fn default() -> Self {
        Self {
            mode: IndexMode::Auto,
            generate_ids: true,
            summary_strategy: SummaryStrategy::Selective {
                min_tokens: 100,
                branch_only: true,
            },
            thinning: ThinningConfig::disabled(),
            optimization: OptimizationConfig {
                enabled: true,
                max_depth: None,
                max_children: None,
                merge_leaf_threshold: 50,
            },
            generate_description: true,
            concurrency: ConcurrencyConfig::default(),
        }
    }
}
```

### IndexMetrics

```rust
/// Performance metrics for indexing
pub struct IndexMetrics {
    /// Parse stage duration (ms)
    pub parse_time_ms: u64,

    /// Build stage duration (ms)
    pub build_time_ms: u64,

    /// Enhance stage duration (ms)
    pub enhance_time_ms: u64,

    /// Total tokens generated (summaries)
    pub total_tokens_generated: usize,

    /// Number of LLM calls
    pub llm_calls: usize,

    /// Number of nodes processed
    pub nodes_processed: usize,

    /// Number of summaries generated
    pub summaries_generated: usize,
}
```

## Incremental Update

### Change Detection

```rust
/// Change detection for incremental updates
pub struct ChangeDetector {
    /// Content hash cache
    hashes: HashMap<String, u64>,

    /// File modification times
    mtimes: HashMap<String, SystemTime>,
}

impl ChangeDetector {
    /// Check if document needs reindexing
    pub fn needs_reindex(&self, doc_id: &str, path: &Path) -> bool {
        // Check mtime first (fast)
        // Then check content hash (accurate)
    }

    /// Detect what changed between old and new trees
    pub fn detect_changes(&self, old: &VectorlessTree, new: &VectorlessTree) -> ChangeSet {
        // Return added/removed/modified nodes
    }
}
```

### Partial Update

```rust
/// Partial tree updater
pub struct PartialUpdater {
    /// Change detector
    detector: ChangeDetector,
}

impl PartialUpdater {
    /// Update only changed portions of the tree
    pub async fn update(
        &self,
        old_tree: &VectorlessTree,
        new_content: &str,
        options: &IndexOptions,
    ) -> Result<UpdateResult> {
        // 1. Parse new content
        // 2. Detect changes
        // 3. Update only affected subtrees
        // 4. Regenerate summaries for changed nodes only
    }
}
```

## Custom Stage Extension

### Plugin Interface

```rust
/// Custom stage plugin
pub trait CustomStage: IndexStage {
    /// Stage priority (lower = earlier)
    fn priority(&self) -> i32 {
        100  // Default priority
    }

    /// Stage dependencies (must run after these)
    fn depends_on(&self) -> Vec<&'static str> {
        vec![]
    }
}

/// Stage registry for plugins
pub struct StageRegistry {
    stages: Vec<Box<dyn IndexStage>>,
}

impl StageRegistry {
    /// Register a custom stage
    pub fn register<S: CustomStage + 'static>(&mut self, stage: S) {
        // Insert at correct position based on priority and dependencies
    }

    /// Build pipeline with registered stages
    pub fn build_pipeline(&self) -> PipelineExecutor {
        // Sort by priority and validate dependencies
    }
}
```

### Example Custom Stage

```rust
/// Example: Custom embedding stage
pub struct EmbeddingStage {
    embedder: EmbeddingClient,
}

impl IndexStage for EmbeddingStage {
    fn name(&self) -> &str {
        "embedding"
    }

    async fn execute(&self, ctx: &mut IndexContext) -> Result<StageResult> {
        let tree = ctx.tree.as_mut().ok_or(Error::NoTree)?;

        for node_id in tree.traverse() {
            if let Some(node) = tree.get(node_id) {
                let embedding = self.embedder.embed(&node.content).await?;
                tree.set_embedding(node_id, embedding);
            }
        }

        Ok(StageResult::success("embedding"))
    }
}

// Usage
let mut registry = StageRegistry::new();
registry.register(EmbeddingStage::new(embedder));
let pipeline = registry.build_pipeline();
```

## Migration Plan

### Phase 1: Core Infrastructure
- Define `IndexStage` trait
- Create `IndexContext` and `IndexMetrics`
- Implement `PipelineExecutor`

### Phase 2: Stage Implementation
- Migrate existing code to stages
- Implement all 6 stages
- Add `Selective` summary strategy

### Phase 3: Advanced Features
- Implement `Lazy` summary strategy with persistence
- Add change detection and partial updates
- Implement custom stage registry

### Phase 4: Integration
- Update `Vectorless` client to use new pipeline
- Add new `IndexOptions` configuration
- Update tests and documentation

## API Examples

### Basic Usage

```rust
// Default configuration
let doc_id = client.index("./document.md").await?;

// Custom options
let options = IndexOptions {
    summary_strategy: SummaryStrategy::Full,
    thinning: ThinningConfig::enabled(500),
    ..Default::default()
};
let doc_id = client.index_with_options("./document.pdf", options).await?;
```

### Incremental Update

```rust
// Check if reindex needed
if client.needs_reindex(&doc_id).await? {
    // Full reindex
    client.reindex(&doc_id).await?;
}

// Or partial update
let changes = client.update_if_changed(&doc_id).await?;
println!("Updated {} nodes", changes.nodes_updated);
```

### Custom Pipeline

```rust
// Create custom pipeline
let mut registry = StageRegistry::new();
registry.register(MyCustomStage::new());

let pipeline = registry.build_pipeline();
let result = pipeline.execute(input).await?;

// Access metrics
println!("Indexing took {}ms", result.metrics.total_time_ms());
println!("Generated {} summaries", result.metrics.summaries_generated);
```
