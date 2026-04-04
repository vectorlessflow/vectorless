# Content Aggregation Design

> Version: 1.0
> Status: Draft
> Last Updated: 2026-04-04

## Overview

Content Aggregation is the final stage of the retrieval pipeline that transforms candidate nodes into structured, relevant content for the user. This document describes the design for a precision-focused, budget-aware content aggregation system.

## Problem Statement

### Current Implementation

The current `aggregate_content` in `JudgeStage` collects content naively:

```
Candidate Node → Node's own content + ALL descendant leaf content
```

### Issues

| Issue | Impact |
|-------|--------|
| **No relevance filtering** | Returns all content from subtree, including irrelevant parts |
| **No token budget** | Large documents may return tens of thousands of tokens |
| **No prioritization** | All leaf content treated equally |
| **Lost structure** | Flat concatenation loses hierarchical context |

## Design Goals

1. **Precision First** - Only return truly relevant content
2. **Budget Aware** - Optimize within token constraints
3. **Structure Aware** - Maintain hierarchical context
4. **Incremental** - Support progressive refinement
5. **Explainable** - Traceable selection decisions

## Architecture

### High-Level Flow

```
┌─────────────────────────────────────────────────────────────┐
│                    Content Aggregator                         │
├─────────────────────────────────────────────────────────────┤
│                                                              │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐      │
│  │   Relevance  │  │    Budget    │  │  Structure   │      │
│  │    Scorer    │─▶│   Allocator  │─▶│   Builder    │      │
│  └──────────────┘  └──────────────┘  └──────────────┘      │
│         ↑                 ↑                 ↑               │
│         │                 │                 │               │
│  ┌──────┴──────┐  ┌──────┴──────┐  ┌──────┴──────┐        │
│  │   Query-    │  │   Token     │  │  Hierarchy  │        │
│  │   Node      │  │   Budget    │  │  Context    │        │
│  │   Scoring   │  │   Config    │  │  Assembly   │        │
│  └─────────────┘  └─────────────┘  └─────────────┘        │
│                                                              │
└─────────────────────────────────────────────────────────────┘
```

### Processing Pipeline

```
Candidate Nodes
      │
      ▼
┌─────────────────┐
│  1. Collect     │  Gather all nodes from candidates + descendants
│     Nodes       │
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│  2. Score       │  Compute relevance score for each content chunk
│     Relevance   │
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│  3. Filter      │  Remove content below relevance threshold
│     by Score    │
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│  4. Allocate    │  Distribute token budget optimally
│     Budget      │
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│  5. Build       │  Assemble structured output
│     Structure   │
└────────┬────────┘
         │
         ▼
    Final Content
```

## Module Design

### 1. RelevanceScorer

Computes fine-grained relevance scores for content.

```rust
pub struct RelevanceScorer {
    query_keywords: Vec<String>,
    strategy: ScoringStrategy,
}

pub enum ScoringStrategy {
    /// Fast keyword matching only
    KeywordOnly,
    /// Keyword + BM25 scoring
    KeywordWithBM25,
    /// Keyword + LLM reranking
    Hybrid { rerank_top_k: usize },
}

pub struct ContentRelevance {
    pub node_id: NodeId,
    pub chunk: ContentChunk,
    pub score: f32,
    pub components: ScoreComponents,
}

pub struct ScoreComponents {
    pub keyword_score: f32,      // Keyword match quality
    pub depth_penalty: f32,      // Distance from candidate node
    pub path_bonus: f32,         // Parent node relevance
    pub density_score: f32,      // Information density
}
```

#### Scoring Formula

```
final_score = (
    keyword_score * 0.50 +
    depth_penalty * 0.20 +
    path_bonus * 0.15 +
    density_score * 0.15
).clamp(0.0, 1.0)

where:
  depth_penalty = 0.9^depth  // 10% penalty per level
  path_bonus = parent_score * 0.2
  density_score = (1 - stopword_ratio) * 0.7 + entity_ratio * 0.3
```

### 2. BudgetAllocator

Distributes token budget across scored content.

```rust
pub struct BudgetAllocator {
    total_budget: usize,
    strategy: AllocationStrategy,
}

pub enum AllocationStrategy {
    /// Select highest-scoring content first
    Greedy,
    /// Distribute proportionally to scores
    Proportional,
    /// Ensure each depth level has representation
    Hierarchical { min_per_level: f32 },
}

pub struct AllocationResult {
    pub selected: Vec<SelectedContent>,
    pub tokens_used: usize,
    pub remaining_budget: usize,
}

pub struct SelectedContent {
    pub node_id: NodeId,
    pub content: String,
    pub tokens: usize,
    pub score: f32,
    pub truncation: Option<TruncationInfo>,
}
```

#### Hierarchical Allocation

```
For each depth level (0 to max_depth):
    1. Sort content by score
    2. Allocate up to min_per_level budget
    3. Continue until level budget exhausted
    4. Move to next level

Benefits:
- Ensures context from all levels
- Prevents shallow-only or deep-only results
- Maintains document structure awareness
```

### 3. StructureBuilder

Assembles selected content into structured output.

```rust
pub struct StructureBuilder {
    format: OutputFormat,
    include_metadata: bool,
}

pub enum OutputFormat {
    Markdown,
    Json,
    Tree,
    Flat,
}

pub struct StructuredContent {
    pub content: String,
    pub structure: Option<ContentTree>,
    pub metadata: ContentMetadata,
}
```

#### Markdown Output Format

```markdown
## Parent Section
Parent content here...

### Child Section A
Child A content here...

### Child Section B
Child B content here...
```

## Configuration

```toml
[retrieval.content]
# Maximum tokens to return
token_budget = 4000

# Minimum relevance score (0.0 - 1.0)
min_relevance_score = 0.3

# Scoring strategy: "keyword_only" | "keyword_bm25" | "hybrid"
scoring_strategy = "keyword_bm25"

# Output format: "markdown" | "json" | "tree"
output_format = "markdown"

# Include relevance scores in output
include_scores = false

# Hierarchical allocation minimum per level
hierarchical_min_per_level = 0.1
```

## Integration Points

### JudgeStage Integration

```rust
impl JudgeStage {
    pub fn with_content_aggregator(mut self, config: ContentAggregatorConfig) -> Self {
        self.content_aggregator = Some(ContentAggregator::new(config));
        self
    }

    fn aggregate_content(&self, ctx: &PipelineContext) -> (String, usize) {
        if let Some(aggregator) = &self.content_aggregator {
            aggregator.aggregate(&ctx.candidates, &ctx.tree, &ctx.query)
        } else {
            // Fallback to legacy behavior
            self.aggregate_content_legacy(ctx)
        }
    }
}
```

### RetrieveOptions Extension

```rust
impl RetrieveOptions {
    pub fn with_content_config(mut self, config: ContentAggregatorConfig) -> Self {
        self.content_config = Some(config);
        self
    }
}
```

## Performance Characteristics

### Latency by Strategy

| Strategy | Latency | Precision | Use Case |
|----------|---------|-----------|----------|
| `KeywordOnly` | ~1ms | Medium | Quick preview |
| `KeywordWithBM25` | ~5ms | High | Default choice |
| `Hybrid` | ~200ms | Highest | Precision queries |

### Memory Usage

- Scorer: O(n) where n = total content length
- Allocator: O(m) where m = number of chunks
- Builder: O(k) where k = selected content size

## Future Enhancements

1. **Semantic Chunking** - Split content by semantic boundaries, not just nodes
2. **LLM Reranking** - Use LLM to rerank top-k chunks
3. **Query-Aware Truncation** - Truncate based on query relevance, not just length
4. **Caching** - Cache aggregation results for repeated queries
5. **Streaming** - Stream content as it's selected

## File Structure

```
src/retrieval/content/
├── mod.rs              # Module entry point
├── aggregator.rs       # Main aggregator logic
├── scorer.rs           # Relevance scoring
├── budget.rs           # Token budget allocation
├── builder.rs          # Structured output building
├── truncation.rs       # Smart truncation utilities
└── config.rs           # Configuration types
```

## Implementation Priority

| Phase | Component | Priority |
|-------|-----------|----------|
| P0 | `RelevanceScorer` (keyword) | High |
| P0 | `BudgetAllocator` (greedy) | High |
| P1 | `StructureBuilder` (markdown) | Medium |
| P1 | BM25 scoring | Medium |
| P2 | Hybrid strategy (LLM rerank) | Low |
| P2 | Caching layer | Low |

## Testing Strategy

### Unit Tests

- Scorer: Test keyword extraction, BM25 calculation, density scoring
- Allocator: Test budget distribution, truncation, edge cases
- Builder: Test output formats, structure preservation

### Integration Tests

- End-to-end aggregation with real documents
- Performance benchmarks
- Token budget compliance

### Quality Metrics

- Precision@k: Relevance of top-k selected chunks
- Recall: Coverage of relevant content
- Latency: P50, P95, P99 response times
