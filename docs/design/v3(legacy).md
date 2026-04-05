# V3 Design: LLM Navigator + Algorithm Collaborative Retrieval

## 🏗️ Architecture Design: LLM + Algorithm Collaborative Retriever Pipeline

### Core Design Principles

```
┌─────────────────────────────────────────────────────────────────┐
│                    Design Philosophy                             │
├─────────────────────────────────────────────────────────────────┤
│  1. Algorithm handles "how to go" - efficient, deterministic,    │
│     low latency                                                  │
│  2. LLM handles "where to go" - semantic understanding,          │
│     ambiguity resolution, direction judgment                     │
│  3. Intervene at key decision points - not every step asks LLM,  │
│     only when needed                                             │
│  4. Layered fallback - algorithm takes over when LLM fails,      │
│     LLM rescues when algorithm fails                             │
└─────────────────────────────────────────────────────────────────┘
```

### Overall Architecture

```
┌─────────────────────────────────────────────────────────────────────────┐
│                          Index Pipeline (Unchanged)                      │
│   Parse → Build → Enhance → Enrich(LLM) → Optimize                      │
└─────────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
                          ┌─────────────────┐
                          │  DocumentTree   │
                          │  + NodeSummary  │
                          └─────────────────┘
                                    │
                                    ▼
┌─────────────────────────────────────────────────────────────────────────┐
│                        Retrieval Pipeline (Enhanced)                     │
│                                                                         │
│  ┌─────────┐    ┌─────────┐    ┌─────────────────────┐    ┌─────────┐  │
│  │ Analyze │───▶│  Plan   │───▶│       Search        │───▶│  Judge  │  │
│  │ (LLM?)  │    │ (LLM?)  │    │  ┌───────────────┐  │    │ (LLM)   │  │
│  └─────────┘    └─────────┘    │  │   Navigator   │  │    └─────────┘  │
│       │              │         │  │ ┌───────────┐ │  │         │       │
│       │              │         │  │ │  LLM +    │ │  │         │       │
│       ▼              ▼         │  │ │ Algorithm │ │  │         ▼       │
│  ┌─────────────────────────┐   │  │ └───────────┘ │  │   ┌───────────┐ │
│  │     LLM Navigator       │◀──┼──┤               │  │   │  NeedMore │ │
│  │  (Key Decision Points)  │   │  │  Search Alg   │  │   │  ◀───────│ │
│  └─────────────────────────┘   │  │  (Greedy/Beam)│  │   └───────────┘ │
│              │                  │  └───────────────┘  │         │       │
│              └──────────────────┴─────────────────────┘         │       │
│                                                                 ▼       │
│                                                         ┌───────────┐   │
│                                                         │ Backtrack │───┘
│                                                         └───────────┘
└─────────────────────────────────────────────────────────────────────────┘
```

---

## 🧭 LLM Navigator Design

### Navigator Responsibilities

Navigator doesn't replace the Search algorithm, but **provides semantic judgment at key decision points**:

```
┌────────────────────────────────────────────────────────────┐
│                    LLM Navigator Responsibilities          │
├────────────────┬───────────────────────────────────────────┤
│     Timing     │                LLM Task                   │
├────────────────┼───────────────────────────────────────────┤
│ Before search  │ Understand query, determine search        │
│ starts         │ starting point and priority directions    │
├────────────────┼───────────────────────────────────────────┤
│ At fork/branch │ When multiple candidate paths exist,      │
│ points         │ judge which is more relevant              │
├────────────────┼───────────────────────────────────────────┤
│ When lost      │ When algorithm is stuck in low-score      │
│                │ paths, provide correction suggestions     │
├────────────────┼───────────────────────────────────────────┤
│ When uncertain │ When algorithm scores are close,          │
│                │ make semantic judgments                    │
├────────────────┼───────────────────────────────────────────┤
│ When           │ Analyze failure reasons, suggest new      │
│ backtracking   │ search directions                          │
└────────────────┴───────────────────────────────────────────┘
```

### Navigator Interface Design

```rust
/// LLM Navigator - Provides semantic navigation at key decision points
pub struct LlmNavigator {
    client: LlmClient,
    config: NavigatorConfig,
}

/// Navigator Configuration
pub struct NavigatorConfig {
    /// Whether to intervene before search starts
    pub guide_at_start: bool,
    /// Whether to intervene at fork points (when candidates > threshold)
    pub guide_at_fork: bool,
    /// Fork point threshold
    pub fork_threshold: usize,
    /// Whether to intervene during backtracking
    pub guide_at_backtrack: bool,
    /// Low score threshold (request LLM intervention when below this value)
    pub low_score_threshold: f32,
    /// Maximum LLM calls (cost control)
    pub max_llm_calls: usize,
}

/// Navigation Guidance
pub struct NavigationGuidance {
    /// Recommended node order (sorted by relevance)
    pub preferred_order: Vec<NodeId>,
    /// Recommended search direction
    pub direction: SearchDirection,
    /// LLM's reasoning process (explainability)
    pub reasoning: String,
    /// Confidence level
    pub confidence: f32,
}

pub enum SearchDirection {
    /// Go deeper into current branch
    GoDeeper,
    /// Explore sibling nodes
    ExploreSiblings,
    /// Backtrack to parent node
    Backtrack,
    /// Jump to a specific node
    JumpTo(NodeId),
    /// Current path is the answer
    ThisIsIt,
}

impl LlmNavigator {
    /// Before search starts: Understand query, determine starting point
    pub async fn guide_start(
        &self,
        tree: &DocumentTree,
        query: &str,
    ) -> Result<StartGuidance>;

    /// At fork point: Choose the best branch
    pub async fn guide_fork(
        &self,
        tree: &DocumentTree,
        current_path: &[NodeId],
        candidates: &[NodeId],
        query: &str,
    ) -> Result<NavigationGuidance>;

    /// During backtracking: Analyze failure, suggest new direction
    pub async fn guide_backtrack(
        &self,
        tree: &DocumentTree,
        failed_path: &[NodeId],
        visited: &HashSet<NodeId>,
        query: &str,
    ) -> Result<NavigationGuidance>;
}
```

---

## 🔄 Search Stage Integration Plan

### New Search Architecture

```rust
/// Enhanced Search Stage - Algorithm + LLM Collaboration
pub struct SearchStage {
    /// Search algorithm
    algorithm: SearchAlgorithm,
    /// LLM Navigator (optional)
    navigator: Option<Arc<LlmNavigator>>,
    /// Configuration
    config: SearchConfig,
}

/// Collaborative Searcher
pub struct CollaborativeSearch {
    /// Underlying search algorithm
    algorithm: Box<dyn SearchTree>,
    /// LLM Navigator
    navigator: LlmNavigator,
    /// Call statistics
    stats: SearchStats,
}

impl CollaborativeSearch {
    pub async fn search(&mut self, tree: &DocumentTree, ctx: &RetrievalContext) -> SearchResult {
        let mut result = SearchResult::default();
        let mut state = SearchState::new(tree.root());

        // 1. Before starting: LLM guides starting point
        if self.navigator.config.guide_at_start {
            let guidance = self.navigator.guide_start(tree, &ctx.query).await?;
            state.apply_guidance(guidance);
        }

        // 2. Search loop
        while !state.is_complete() {
            // 2.1 Algorithm selects candidates
            let candidates = self.algorithm.select_candidates(tree, &state);

            // 2.2 Determine if LLM consultation is needed
            if self.should_consult_llm(&candidates, &state) {
                let guidance = self.navigator.guide_fork(
                    tree,
                    &state.path,
                    &candidates,
                    &ctx.query
                ).await?;

                // 2.3 Re-rank candidates using LLM suggestions
                state.candidates = self.merge_algorithm_and_llm(
                    candidates,
                    guidance
                );
            }

            // 2.4 Algorithm executes next step
            self.algorithm.step(tree, &mut state);

            // 2.5 Check if backtracking is needed
            if state.needs_backtrack() {
                if self.navigator.config.guide_at_backtrack {
                    let guidance = self.navigator.guide_backtrack(
                        tree,
                        &state.path,
                        &state.visited,
                        &ctx.query
                    ).await?;
                    state.apply_backtrack_guidance(guidance);
                } else {
                    state.backtrack();
                }
            }

            self.stats.iterations += 1;
        }

        result
    }

    /// Determine whether to consult LLM
    fn should_consult_llm(&self, candidates: &[NodeId], state: &SearchState) -> bool {
        // Condition 1: Candidate count exceeds threshold (fork point)
        if candidates.len() > self.navigator.config.fork_threshold {
            return true;
        }

        // Condition 2: Candidate scores are close (algorithm cannot distinguish)
        if self.scores_are_close(candidates) {
            return true;
        }

        // Condition 3: Current score is too low (might be wrong direction)
        if state.best_score < self.navigator.config.low_score_threshold {
            return true;
        }

        // Condition 4: Haven't exceeded LLM call limit
        self.stats.llm_calls < self.navigator.config.max_llm_calls
    }
}
```

---

## 📊 LLM Intervention Points in Pipeline Stages

```
┌─────────────────────────────────────────────────────────────────────────┐
│                         Retrieval Pipeline                               │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                         │
│  Analyze Stage                                                          │
│  ┌─────────────────────────────────────────────────────────────────┐   │
│  │  [Algorithm] Keyword extraction, complexity estimation           │   │
│  │  [LLM]      Optional: Deep semantic analysis, intent detection   │   │
│  └─────────────────────────────────────────────────────────────────┘   │
│                              │                                          │
│                              ▼                                          │
│  Plan Stage                                                             │
│  ┌─────────────────────────────────────────────────────────────────┐   │
│  │  [Algorithm] Select strategy based on complexity                 │   │
│  │              (keyword/llm/semantic)                              │   │
│  │  [LLM]      Optional: Strategy recommendation for complex        │   │
│  │              queries                                             │   │
│  └─────────────────────────────────────────────────────────────────┘   │
│                              │                                          │
│                              ▼                                          │
│  Search Stage ◀━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┓   │
│  ┌─────────────────────────────────────────────────────────────────┐   │
│  │                                                                  │   │
│  │  ┌─────────────┐     ┌─────────────────────────────────────┐    │   │
│  │  │  Algorithm  │────▶│           LLM Navigator              │    │   │
│  │  │  (Primary)  │     │  ┌─────────────────────────────┐    │    │   │
│  │  │             │     │  │ guide_start()    Start guide │    │    │   │
│  │  │ - Greedy    │◀───▶│  │ guide_fork()     Fork choice │    │    │   │
│  │  │ - Beam      │     │  │ guide_backtrack()Backtrack   │    │    │   │
│  │  │ - MCTS      │     │  └─────────────────────────────┘    │    │   │
│  │  │             │     │                                     │    │   │
│  │  └─────────────┘     └─────────────────────────────────────┘    │   │
│  │                                                                  │   │
│  └─────────────────────────────────────────────────────────────────┘   │
│                              │                                          │
│                              ▼                                          │
│  Judge Stage                                                            │
│  ┌─────────────────────────────────────────────────────────────────┐   │
│  │  [Algorithm] Token count check, threshold judgment               │   │
│  │  [LLM]      Content sufficiency judgment, answer completeness    │   │
│  │              evaluation                                           │   │
│  └─────────────────────────────────────────────────────────────────┘   │
│                              │                                          │
│                              ▼                                          │
│                      ┌───────────────┐                                  │
│                      │  Sufficient?  │─── No ──▶ Backtrack ──┐         │
│                      └───────────────┘                         │         │
│                              │ Yes                            │         │
│                              ▼                                 │         │
│                      ┌───────────────┐                        │         │
│                      │    Result     │◀───────────────────────┘         │
│                      └───────────────┘                                  │
└─────────────────────────────────────────────────────────────────────────┘
```

---

## 🎯 Implementation Plan

### Phase 1: Basic Integration (1-2 weeks)

```rust
// 1. Define Navigator trait and basic implementation
pub trait Navigator: Send + Sync {
    async fn guide_fork(&self, ctx: &NavigationContext) -> NavigationGuidance;
}

// 2. Integrate into SearchStage
pub struct SearchStage {
    algorithm: SearchAlgorithm,
    navigator: Option<Arc<dyn Navigator>>,  // New
}

// 3. Modify search loop to call navigator at fork points
```

### Phase 2: Enhanced Capabilities (2-3 weeks)

```rust
// 1. Implement complete LlmNavigator
// 2. Add guide_start, guide_backtrack
// 3. Implement intelligent intervention judgment logic
// 4. Add caching (same query + same context → cached result)
```

### Phase 3: Optimization and Monitoring (1-2 weeks)

```rust
// 1. Add A/B testing capability (pure algorithm vs algorithm+LLM)
// 2. Add cost control (max_llm_calls, budget)
// 3. Add effectiveness monitoring (retrieval accuracy, latency, cost)
// 4. Adaptive intervention (dynamically adjust intervention frequency
//    based on historical effectiveness)
```

---

## 📁 Suggested Code Structure

```
src/retrieval/
├── mod.rs
├── pipeline/
│   ├── mod.rs
│   ├── stage.rs
│   ├── orchestrator.rs
│   └── context.rs
├── stages/
│   ├── analyze.rs
│   ├── plan.rs
│   ├── search.rs          # Integrate Navigator
│   └── judge.rs
├── search/
│   ├── mod.rs
│   ├── trait.rs
│   ├── greedy.rs
│   ├── beam.rs
│   └── mcts.rs
├── navigator/              # New module
│   ├── mod.rs
│   ├── trait.rs            # Navigator trait
│   ├── llm_navigator.rs    # LLM implementation
│   ├── noop_navigator.rs   # No-op implementation
│   ├── guidance.rs         # NavigationGuidance types
│   └── config.rs           # NavigatorConfig
├── strategy/
│   ├── mod.rs
│   ├── keyword.rs
│   ├── llm.rs
│   └── semantic.rs
```

---

## 🤔 Key Questions

### Q1: Difference between Navigator and Strategy?

|                    | Strategy                    | Navigator                      |
|--------------------|-----------------------------|--------------------------------|
| Granularity        | Single node evaluation      | Global navigation suggestion   |
| Input              | Single node information     | Path + candidates + context    |
| Output             | Score (0-1)                 | Direction + ranking + reasoning|
| Call frequency     | Every candidate node        | Key decision points            |

### Q2: How to control LLM call costs?

```rust
pub struct CostControl {
    /// Maximum LLM calls per retrieval
    max_calls_per_query: usize,
    /// Daily budget
    daily_budget: Option<Money>,
    /// Only call when confidence is low
    min_uncertainty: f32,
}
```

### Q3: How to evaluate effectiveness?

```rust
pub struct RetrievalMetrics {
    /// Retrieval precision
    pub precision: f32,
    /// Retrieval recall
    pub recall: f32,
    /// LLM call count
    pub llm_calls: usize,
    /// Total latency
    pub latency_ms: u64,
    /// Cost
    pub cost: Money,
}
```
