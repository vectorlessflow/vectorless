这里是为您翻译的英文版 Pilot 设计文档。

# Pilot Design Document

> Pilot - The Brain of the Retriever Pipeline

## Overview

Pilot is the core intelligent component of the Vectorless retrieval system. It is responsible for understanding queries, analyzing document structures, and making search decisions. Unlike traditional vector retrieval, Pilot uses LLMs for semantic understanding and navigation decisions while maintaining efficient algorithmic execution.

### Design Philosophy

```
┌─────────────────────────────────────────────────────────────────┐
│                        Design Philosophy                        │
├─────────────────────────────────────────────────────────────────┤
│  1. Algorithm handles "How to walk" - Efficient, deterministic, low latency                      │
│  2. Pilot handles "Where to go" - Semantic understanding, ambiguity resolution, direction judgment │
│  3. Key decision point intervention - Not asking the LLM at every step, but only when needed     │
│  4. Layered fallback - Algorithm takes over when LLM fails, Pilot rescues when algorithm fails  │
└─────────────────────────────────────────────────────────────────┘
```

### Naming Origin

**Pilot** - Like the pilot of an airplane, Pilot does not directly operate every mechanical part (that is the Algorithm's responsibility), but is responsible for:
- Understanding the destination (User Query)
- Planning the route (Search Strategy)
- Making decisions at key nodes (Intervention Points)
- Responding to emergencies (Fallback)

---

## 1. Pilot Detailed Design

### 1.1 Overall Architecture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                              Pilot Architecture                             │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  ┌───────────────────────────────────────────────────────────────────────┐  │
│  │                           Pilot (Core)                                 │  │
│  │                                                                       │  │
│  │   ┌─────────────┐   ┌─────────────┐   ┌─────────────┐                │  │
│  │   │   Query     │   │  Context    │   │  Decision   │                │  │
│  │   │  Analyzer   │──▶│   Builder   │──▶│   Engine    │                │  │
│  │   │  (Query Analyzer)│  │ (Context Builder)│  │ (Decision Engine)│     │  │
│  │   └─────────────┘   └─────────────┘   └──────┬──────┘                │  │
│  │                                              │                        │  │
│  │                                              ▼                        │  │
│  │   ┌─────────────┐   ┌─────────────┐   ┌─────────────┐                │  │
│  │   │   Response  │◀──│     LLM     │◀──│   Prompt    │                │  │
│  │   │   Parser    │   │   Client    │   │   Builder   │                │  │
│  │   │ (Response Parser)│  │  (LLM Client)  │  │ (Prompt Builder) │     │  │
│  │   └─────────────┘   └─────────────┘   └─────────────┘                │  │
│  │                                                                       │  │
│  └───────────────────────────────────────────────────────────────────────┘  │
│                                                                             │
│  ┌───────────────────────────────────────────────────────────────────────┐  │
│  │                        Supporting Systems                              │  │
│  │                                                                       │  │
│  │   ┌─────────────┐   ┌─────────────┐   ┌─────────────┐                │  │
│  │   │   Budget    │   │    Fallback │   │   Metrics   │                │  │
│  │   │  Controller │   │   Manager   │   │  Collector  │                │  │
│  │   │ (Budget Controller)│ (Fallback Manager)│ (Metrics Collector)│   │  │
│  │   └─────────────┘   └─────────────┘   └─────────────┘                │  │
│  │                                                                       │  │
│  │   ┌─────────────┐   ┌─────────────┐   ┌─────────────┐                │  │
│  │   │   Policy    │   │    Cache    │   │   Logger    │                │  │
│  │   │   Manager   │   │  (Optional) │   │  (Tracing)  │                │  │
│  │   │ (Policy Manager)│  │   (Cache)     │  │ (Logger/Tracing) │      │  │
│  │   └─────────────┘   └─────────────┘   └─────────────┘                │  │
│  │                                                                       │  │
│  └───────────────────────────────────────────────────────────────────────┘  │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## 1.4 Information Sources for Pilot Decisions

Pilot's decisions rely on multi-layered information, with the TOC View being the core—it is like a navigation electronic map.

### Information Source Architecture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                      Pilot's "Navigation Map"                                │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│                         ┌─────────────────┐                                │
│                         │   User Query    │                                │
│                         │   "PostgreSQL   │                                │
│                         │   Connection Pool Config"│                       │
│                         └────────┬────────┘                                │
│                                  │                                          │
│                                  ▼                                          │
│  ┌───────────────────────────────────────────────────────────────────────┐  │
│  │                        Pilot Context                                   │  │
│  │                                                                       │  │
│  │   ┌─────────────┐   ┌─────────────┐   ┌─────────────┐                │  │
│  │   │  TOC View   │   │ Current     │   │ Candidates  │                │  │
│  │   │ (E-Map)     │   │ Path        │   │ Info        │                │  │
│  │   │             │   │ (Current Pos)│   │ (Candidates)│                │  │
│  │   └──────┬──────┘   └──────┬──────┘   └──────┬──────┘                │  │
│  │          │                 │                 │                        │  │
│  │          └─────────────────┼─────────────────┘                        │  │
│  │                            ▼                                          │  │
│  │                   ┌─────────────────┐                                 │  │
│  │                   │   LLM Decision  │                                 │  │
│  │                   │   (Where to go) │                                 │  │
│  │                   └─────────────────┘                                 │  │
│  │                                                                       │  │
│  └───────────────────────────────────────────────────────────────────────┘  │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### TOC View - Electronic Map (Core)

The TOC View is the core basis for Pilot's decisions, built from content generated during the Index phase:

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                      TOC View - Electronic Map                              │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  Content generated during Index phase:                                      │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │  TreeNode {                                                         │   │
│  │    title: "Configuration",       // Title                          │   │
│  │    summary: "This chapter introduces...", // LLM-generated Summary │   │
│  │    depth: 1,                                                        │   │
│  │    children: [...],                                                 │   │
│  │  }                                                                  │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
│  TOC View Construction Logic:                                               │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │  generate_toc_view(tree, current_node):                             │   │
│  │                                                                     │   │
│  │    // 1. Generate from current node perspective                      │   │
│  │    // 2. Include sibling nodes (horizontal view)                     │   │
│  │    // 3. Include child nodes (vertical view)                        │   │
│  │    // 4. Each node contains title + summary                         │   │
│  │                                                                     │   │
│  │  Example Output:                                                     │   │
│  │  ┌─────────────────────────────────────────────────────────────┐   │   │
│  │  │  📍 Current Location: Root → Configuration                  │   │   │
│  │  │                                                             │   │   │
│  │  │  📂 Sibling Nodes:                                          │   │   │
│  │  │  ├─ Introduction [Overview of features and architecture]    │   │   │
│  │  │  ├─ Installation [Installation steps and requirements]      │   │   │
│  │  │  ├─ Configuration ⭐ [Detailed config items]  ← Current    │   │   │
│  │  │  │   ├─ Basic Config [Basic parameter settings]            │   │   │
│  │  │  │   ├─ Database Config [DB connection related] ← Match!   │   │   │
│  │  │  │   └─ Advanced Config [Performance tuning options]        │   │   │
│  │  │  └─ API Reference [Interface documentation]                 │   │   │
│  │  └─────────────────────────────────────────────────────────────┘   │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Three-Layer Information Structure

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                       Three Layers of Pilot Decision Info                   │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  Layer 1: TOC View (Global Map)                                             │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │  Role: Provides a global structural view of the document            │   │
│  │  Source: Summary generated by the Enrich stage of the Index Pipeline│   │
│  │  Token: ~200-500 tokens                                             │   │
│  │                                                                     │   │
│  │  Example:                                                           │   │
│  │  "Doc Structure: 1.Intro 2.Install 3.Config(3.1Basic 3.2DB 3.3Adv) 4.API"│
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
│  Layer 2: Current Path (Current Location)                                   │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │  Role: Tells the LLM where we have been                             │   │
│  │  Source: Path records of the search process                         │   │
│  │  Token: ~50-100 tokens                                              │   │
│  │                                                                     │   │
│  │  Example:                                                           │   │
│  │  "Current Path: Root → Configuration → Database Config"             │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
│  Layer 3: Candidates Detail (Candidate Intersection Details)               │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │  Role: Provides detailed info on candidate nodes for LLM judgment   │   │
│  │  Source: TreeNode's title + summary + partial content               │   │
│  │  Token: ~100-300 tokens                                            │   │
│  │                                                                     │   │
│  │  Example:                                                           │   │
│  │  Candidates:                                                        │   │
│  │  A. Connection String                                               │   │
│  │     Summary: Configure DB connection URL and auth info             │   │
│  │  B. Connection Pool ⭐                                              │   │
│  │     Summary: Configure pool size, timeouts, max connections, etc.  │   │
│  │  C. Timeout Settings                                                 │   │
│  │     Summary: Configure query and connection timeout                 │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Decision Process Example

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                          Pilot Decision Process Example                     │
└─────────────────────────────────────────────────────────────────────────────┘

Query: "How to configure the max connections for PostgreSQL connection pool?"

Step 1: Build TOC View (from Index stage summary)
┌─────────────────────────────────────────────────────────────────────────────┐
│  TOC View (Simplified):                                                    │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │  Document Structure:                                               │   │
│  │  1. Quick Start                                                    │   │
│  │  2. Configuration                                                  │   │
│  │     2.1 Basic Config                                               │   │
│  │     2.2 Database Config                                            │   │
│  │         - Connection String                                        │   │
│  │         - Connection Pool ← Contains "Connection Pool"            │   │
│  │         - Timeout Settings                                         │   │
│  │     2.3 Advanced Config                                            │   │
│  │  3. API                                                            │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
│   This TOC is constructed from Index stage LLM-generated summaries!       │
└─────────────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
Step 2: LLM Analysis
┌─────────────────────────────────────────────────────────────────────────────┐
│  Information seen by LLM:                                                   │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │  User Query: "How to configure the max connections for PostgreSQL connection pool?" │
│  │                                                                     │   │
│  │  Current Location: Configuration → Database Config                 │   │
│  │                                                                     │   │
│  │  Candidates:                                                        │   │
│  │  1. Connection String [Configure DB URL and auth]                  │   │
│  │  2. Connection Pool [Configure pool size, timeout, max connections] ← Direct Match! │
│  │  3. Timeout Settings [Configure query timeout]                     │   │
│  │                                                                     │   │
│  │  Which node is most likely to contain the answer?                  │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
│  LLM Reasoning:                                                             │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │  Query Keywords: "Connection Pool", "Max Connections"               │   │
│  │  Candidate 2 Summary: "Connection Pool", "Max Connections"          │   │
│  │  → Candidate 2 matches directly, Confidence 0.95                   │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
Step 3: Return Decision
┌─────────────────────────────────────────────────────────────────────────────┐
│  PilotDecision {                                                           │
│    ranked_candidates: [                                                    │
│      (Node 2 "Connection Pool", score: 0.95, reason: "Summary directly matches query keywords"), │
│      (Node 3 "Timeout Settings", score: 0.30, reason: "Not very relevant"),                  │
│      (Node 1 "Connection String", score: 0.20, reason: "Irrelevant"),                  │
│    ],                                                                      │
│    direction: GoDeeper,                                                    │
│    confidence: 0.95,                                                       │
│    reasoning: "Candidate node 'Connection Pool' summary explicitly mentions 'max connections', direct query match", │
│  }                                                                         │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Key Insights

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                           Key Insights                                      │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  1. Index stage summary quality determines Pilot effectiveness             │
│     ┌─────────────────────────────────────────────────────────────────┐    │
│     │   Good summary: "Configure connection pool size, timeout, max connections params" │
│     │   Bad summary: "This chapter introduces connection pool related content" │
│     │                                                                 │    │
│     │  → The prompt in the Index Enrich stage is crucial!             │    │
│     └─────────────────────────────────────────────────────────────────┘    │
│                                                                             │
│  2. TOC View needs to be generated dynamically                             │
│     ┌─────────────────────────────────────────────────────────────────┐    │
│     │  Not the TOC of the entire document, but a local view from the "current node" perspective │
│     │  Includes: Sibling nodes + Child nodes + Parent chain           │    │
│     │                                                                 │    │
│     │  This keeps Token consumption manageable while providing context│    │
│     └─────────────────────────────────────────────────────────────────┘    │
│                                                                             │
│  3. Analogy: Gaode Map (or Google Maps) Navigation                         │
│     ┌─────────────────────────────────────────────────────────────────┐    │
│     │  TOC View     = Map (Road network)                              │    │
│     │  Summary      = Road signs (Intersection descriptions)           │    │
│     │  Current Path = GPS Location (Current position)                 │    │
│     │  Candidates   = Upcoming intersections (Optional directions)   │    │
│     │  Query        = Destination (Where to go)                       │    │
│     │                                                                 │    │
│     │  Pilot        = Driver (Integrates above info to make decisions)│    │
│     └─────────────────────────────────────────────────────────────────┘    │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### ContextBuilder Token Budget Allocation

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                    ContextBuilder - Token Budget Allocation                  │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  Token Budget Allocation (Assuming 500 tokens total budget):                │
│                                                                             │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │                                                                     │   │
│  │  ┌────────────────────────────────────────┐  30% (150 tokens)      │   │
│  │  │  Query + Intent                        │                        │   │
│  │  │  "PostgreSQL connection pool max connections config"│           │   │
│  │  └────────────────────────────────────────┘                        │   │
│  │                                                                     │   │
│  │  ┌────────────────────────────────────────────────┐  20% (100 tokens) │   │
│  │  │  Current Path                          │                        │   │
│  │  │  Root → Configuration → Database Config       │                        │   │
│  │  └────────────────────────────────────────┘                        │   │
│  │                                                                     │   │
│  │  ┌────────────────────────────────────────────────┐  40% (200 tokens) │   │
│  │  │  Candidates (title + summary each)     │                        │   │
│  │  │  A. Connection String [Configure URL and auth] │                        │   │
│  │  │  B. Connection Pool [Configure pool size, max connections] │    │   │
│  │  │  C. Timeout Settings [Configure timeout]            │                        │   │
│  │  └────────────────────────────────────────┘                        │   │
│  │                                                                     │   │
│  │  ┌────────────────────────────────────────────────┐  10% (50 tokens)  │   │
│  │  │  Sibling Context (Sibling overview)        │                        │   │
│  │  │  Other siblings: Basic Config, Advanced Config │                        │   │
│  │  └────────────────────────────────────────┘                        │   │
│  │                                                                     │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
│  Dynamic Adjustment Strategy:                                              │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │  if candidates.len() > 5:                                           │   │
│  │      // Too many candidates, reduce detail per candidate            │   │
│  │      Include only title, exclude summary                            │   │
│  │                                                                     │   │
│  │  if depth > 3:                                                      │   │
│  │      // Deep search, reduce TOC range                               │   │
│  │      Show only current layer and child layers                        │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## 2. Intervention Point Detailed Design

### 2.1 Intervention Point Types

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                          Pilot Intervention Points                          │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │  START - Search Start                                               │   │
│  ├─────────────────────────────────────────────────────────────────────┤   │
│  │  Timing: Before search algorithm starts                             │   │
│  │  Task: Understand query intent, determine entry points and priority  │   │
│  │  Input: query, tree (ToC view)                                      │   │
│  │  Output: entry_points, initial_direction, confidence                │   │
│  │  Config: guide_at_start: bool                                        │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │  FORK - Fork in the Road                                            │   │
│  ├─────────────────────────────────────────────────────────────────────┤   │
│  │  Timing: When current node has multiple candidate child nodes       │   │
│  │  Task: Determine which branch is more likely to contain the answer  │   │
│  │  Input: path, candidates, query                                     │   │
│  │  Output: ranked_candidates, direction, confidence                    │   │
│  │  Trigger: candidates.len() > fork_threshold                         │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │  BACKTRACK - Backtrack                                             │   │
│  ├─────────────────────────────────────────────────────────────────────┤   │
│  │  Timing: When Judge determines content is insufficient, needs backtracking │
│  │  Task: Analyze failure reason, suggest new search direction        │   │
│  │  Input: failed_path, visited, query                                 │   │
│  │  Output: alternative_branches, backtrack_reason                    │   │
│  │  Config: guide_at_backtrack: bool                                   │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │  EVALUATE - Node Evaluation                                        │   │
│  ├─────────────────────────────────────────────────────────────────────┤   │
│  │  Timing: When needing to determine if current node contains answer │   │
│  │  Task: Evaluate relevance of node content to query                  │   │
│  │  Input: node_content, query                                         │   │
│  │  Output: relevance_score, is_answer, reasoning                      │   │
│  │  Trigger: Reaching leaf node or when algorithm is uncertain         │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 2.2 Intervention Judgment Logic

```rust
impl Pilot for LlmPilot {
    fn should_intervene(&self, state: &SearchState<'_>) -> bool {
        let config = &self.config.intervention;
        
        // Condition 1: Budget check (Highest priority)
        if !self.budget.can_call() {
            return false;
        }
        
        // Condition 2: Number of candidates exceeds threshold (Fork)
        if state.candidates.len() > config.fork_threshold {
            return true;
        }
        
        // Condition 3: Candidate scores are close (Algorithm cannot distinguish)
        if self.scores_are_close(state.candidates, state.tree, config.score_gap_threshold) {
            return true;
        }
        
        // Condition 4: Current score is too low (May be going the wrong way)
        if state.best_score < config.low_score_threshold {
            return true;
        }
        
        // Condition 5: During backtracking and config allows
        if state.is_backtracking && self.config.guide_at_backtrack {
            return true;
        }
        
        // Condition 6: Intervention limit per level
        let level_calls = self.get_level_calls(state.depth);
        if level_calls >= config.max_interventions_per_level {
            return false;
        }
        
        false
    }
}

/// Check if candidate scores are close
fn scores_are_close(&self, candidates: &[NodeId], tree: &DocumentTree, threshold: f32) -> bool {
    if candidates.len() < 2 {
        return false;
    }
    
    let scores: Vec<f32> = candidates.iter()
        .map(|&id| self.scorer.quick_score(tree, id))
        .collect();
    
    let max_score = scores.iter().cloned().fold(0.0, f32::max);
    let min_score = scores.iter().cloned().fold(1.0, f32::min);
    
    (max_score - min_score) < threshold
}
```

### 2.3 Intervention Configuration

```rust
/// Intervention Configuration
#[derive(Debug, Clone)]
pub struct InterventionConfig {
    /// Candidate count threshold (Consider intervention if exceeded)
    pub fork_threshold: usize,
    /// Score gap threshold (Intervene if gap is smaller than this)
    pub score_gap_threshold: f32,
    /// Low score threshold (Intervene if highest score is lower than this)
    pub low_score_threshold: f32,
    /// Max interventions per level
    pub max_interventions_per_level: usize,
}

impl Default for InterventionConfig {
    fn default() -> Self {
        Self {
            fork_threshold: 3,           // Intervene when > 3 candidates
            score_gap_threshold: 0.15,  // Intervene if score gap < 0.15
            low_score_threshold: 0.3,    // Intervene if score < 0.3
            max_interventions_per_level: 2,  // Max 2 interventions per level
        }
    }
}
```

---

## 3. Fallback Mechanism

### 3.1 Fallback Levels

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                          Fallback Levels                                    │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  Level 0: Normal LLM Call                                                   │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │  Condition: Budget sufficient, LLM service available                │   │
│  │  Behavior: Normal LLM call, get decision                           │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                          │ Failure                                        │
│                          ▼                                                │
│  Level 1: Retry                                                           │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │  Condition: Network error, timeout, rate limit                      │   │
│  │  Behavior: Exponential backoff retry, max 3 times                  │   │
│  │  Params: initial_delay=1s, max_delay=10s, max_attempts=3           │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                          │ Failure                                        │
│                          ▼                                                │
│  Level 2: Simplify Context                                                │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │  Condition: Token limit exceeded, context too long                  │   │
│  │  Behavior: Reduce context info, keep only core content              │   │
│  │  Strategy: Remove ToC, keep only current node and candidate titles  │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                          │ Failure                                        │
│                          ▼                                                │
│  Level 3: Pure Algorithm Mode                                             │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │  Condition: LLM completely unavailable, budget exhausted            │   │
│  │  Behavior: Rely entirely on algorithm scoring, no LLM calls         │   │
│  │  Result: Use NodeScorer keyword matching                            │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 3.2 Fallback Strategy Definition

```rust
/// Fallback Strategy
#[derive(Debug, Clone)]
pub enum FallbackStrategy {
    /// Retry strategy
    Retry {
        max_attempts: usize,
        backoff: BackoffPolicy,
    },
    /// Simplify context
    SimplifyContext {
        remove_toc: bool,
        max_candidates: usize,
    },
    /// Use algorithm instead
    UseAlgorithm,
    /// Return default decision
    ReturnDefault,
}

/// Backoff Policy
#[derive(Debug, Clone)]
pub enum BackoffPolicy {
    /// Fixed interval
    Fixed { delay_ms: u64 },
    /// Linear increase
    Linear { initial_ms: u64, increment_ms: u64 },
    /// Exponential increase
    Exponential { initial_ms: u64, multiplier: f64, max_ms: u64 },
}

impl Default for BackoffPolicy {
    fn default() -> Self {
        Self::Exponential {
            initial_ms: 1000,
            multiplier: 2.0,
            max_ms: 10000,
        }
    }
}
```

### 3.3 FallbackManager Implementation

```rust
/// Fallback Manager
pub struct FallbackManager {
    config: FallbackConfig,
    /// Current fallback level
    current_level: AtomicU8,
    /// Consecutive failure count
    consecutive_failures: AtomicUsize,
}

impl FallbackManager {
    /// Execute with fallback
    pub async fn execute_with_fallback<F, T>(
        &self,
        operation: F,
    ) -> Result<T, FallbackError>
    where
        F: Fn() -> std::pin::Pin<Box<dyn Future<Output = Result<T, PilotError>> + Send>>,
    {
        let mut level = self.current_level.load(Ordering::Relaxed);
        
        loop {
            match level {
                0 => {
                    // Level 0: Normal call
                    match operation().await {
                        Ok(result) => {
                            self.on_success();
                            return Ok(result);
                        }
                        Err(e) => {
                            self.on_failure();
                            if self.should_escalate() {
                                level = 1;
                                continue;
                            }
                            return Err(FallbackError::from(e));
                        }
                    }
                }
                1 => {
                    // Level 1: Retry
                    match self.retry_operation(&operation).await {
                        Ok(result) => {
                            self.on_success();
                            return Ok(result);
                        }
                        Err(_) => {
                            level = 2;
                            continue;
                        }
                    }
                }
                2 => {
                    // Level 2: Simplify context
                    // Handled by caller, return specific error
                    return Err(FallbackError::SimplifyContextRequired);
                }
                3 => {
                    // Level 3: Pure algorithm mode
                    return Err(FallbackError::AlgorithmFallback);
                }
                _ => unreachable!(),
            }
        }
    }
    
    /// Retry operation
    async fn retry_operation<F, T>(&self, operation: &F) -> Result<T, PilotError>
    where
        F: Fn() -> std::pin::Pin<Box<dyn Future<Output = Result<T, PilotError>> + Send>>,
    {
        let policy = &self.config.retry_policy;
        let mut delay = policy.initial_delay_ms();
        
        for attempt in 0..policy.max_attempts {
            if attempt > 0 {
                tokio::time::sleep(Duration::from_millis(delay)).await;
                delay = policy.next_delay(delay);
            }
            
            match operation().await {
                Ok(result) => return Ok(result),
                Err(e) if attempt == policy.max_attempts - 1 => return Err(e),
                Err(_) => continue,
            }
        }
        
        Err(PilotError::RetryExhausted)
    }
    
    fn on_success(&self) {
        self.consecutive_failures.store(0, Ordering::Relaxed);
        // Gradually recover to higher level
        let current = self.current_level.load(Ordering::Relaxed);
        if current > 0 {
            self.current_level.fetch_sub(1, Ordering::Relaxed);
        }
    }
    
    fn on_failure(&self) {
        let failures = self.consecutive_failures.fetch_add(1, Ordering::Relaxed);
        // Escalate fallback level after 3 consecutive failures
        if failures >= 2 {
            let current = self.current_level.load(Ordering::Relaxed);
            if current < 3 {
                self.current_level.fetch_add(1, Ordering::Relaxed);
            }
            self.consecutive_failures.store(0, Ordering::Relaxed);
        }
    }
    
    fn should_escalate(&self) -> bool {
        self.consecutive_failures.load(Ordering::Relaxed) >= 3
    }
}
```

---

## 4. Token Consumption Measurement

### 4.1 Budget Configuration

```rust
/// Budget Configuration
#[derive(Debug, Clone)]
pub struct BudgetConfig {
    /// Max tokens per single query retrieval
    pub max_tokens_per_query: usize,
    /// Max tokens per single LLM call
    pub max_tokens_per_call: usize,
    /// Max LLM calls per single query
    pub max_calls_per_query: usize,
    /// Max calls per level (depth)
    pub max_calls_per_level: usize,
    /// Hard limit flag (true: reject if over budget; false: try to continue)
    pub hard_limit: bool,
}

impl Default for BudgetConfig {
    fn default() -> Self {
        Self {
            max_tokens_per_query: 2000,   // Max 2000 tokens per query
            max_tokens_per_call: 500,     // Max 500 tokens per call
            max_calls_per_query: 5,       // Max 5 calls
            max_calls_per_level: 2,      // Max 2 calls per level
            hard_limit: true,
        }
    }
}
```

### 4.2 Budget Controller

```rust
/// Budget Controller
pub struct BudgetController {
    config: BudgetConfig,
    /// Tokens used
    tokens_used: AtomicUsize,
    /// Calls made
    calls_made: AtomicUsize,
    /// Calls per level
    level_calls: RwLock<HashMap<usize, usize>>,
}

impl BudgetController {
    /// Create new budget controller
    pub fn new(config: BudgetConfig) -> Self {
        Self {
            config,
            tokens_used: AtomicUsize::new(0),
            calls_made: AtomicUsize::new(0),
            level_calls: RwLock::new(HashMap::new()),
        }
    }
    
    /// Check if LLM can be called
    pub fn can_call(&self) -> bool {
        let calls = self.calls_made.load(Ordering::Relaxed);
        let tokens = self.tokens_used.load(Ordering::Relaxed);
        
        calls < self.config.max_calls_per_query
            && tokens < self.config.max_tokens_per_query
    }
    
    /// Check if call is possible at specific level
    pub fn can_call_at_level(&self, level: usize) -> bool {
        if !self.can_call() {
            return false;
        }
        
        let level_calls = self.level_calls.read().unwrap();
        let calls = level_calls.get(&level).copied().unwrap_or(0);
        calls < self.config.max_calls_per_level
    }
    
    /// Estimate call cost
    pub fn estimate_cost(&self, context: &str) -> usize {
        // Use tiktoken or simple character estimation
        // Rough estimate: 1 token ≈ 4 chars (English) or 1.5 chars (Chinese)
        let char_count = context.chars().count();
        // Conservative estimate, based on Chinese
        char_count / 2 + 100  // +100 reserved for output
    }
    
    /// Check if estimated cost is within budget
    pub fn can_afford(&self, estimated_cost: usize) -> bool {
        let remaining = self.remaining_budget();
        estimated_cost <= remaining && estimated_cost <= self.config.max_tokens_per_call
    }
    
    /// Get remaining budget
    pub fn remaining_budget(&self) -> usize {
        let used = self.tokens_used.load(Ordering::Relaxed);
        self.config.max_tokens_per_query.saturating_sub(used)
    }
    
    /// Record token usage
    pub fn record_usage(&self, input_tokens: usize, output_tokens: usize, level: usize) {
        let total = input_tokens + output_tokens;
        self.tokens_used.fetch_add(total, Ordering::Relaxed);
        self.calls_made.fetch_add(1, Ordering::Relaxed);
        
        // Record level calls
        let mut level_calls = self.level_calls.write().unwrap();
        *level_calls.entry(level).or_insert(0) += 1;
    }
    
    /// Get usage statistics
    pub fn get_usage_stats(&self) -> BudgetUsage {
        BudgetUsage {
            tokens_used: self.tokens_used.load(Ordering::Relaxed),
            calls_made: self.calls_made.load(Ordering::Relaxed),
            max_tokens: self.config.max_tokens_per_query,
            max_calls: self.config.max_calls_per_query,
        }
    }
    
    /// Reset (when new query starts)
    pub fn reset(&self) {
        self.tokens_used.store(0, Ordering::Relaxed);
        self.calls_made.store(0, Ordering::Relaxed);
        self.level_calls.write().unwrap().clear();
    }
}

/// Budget Usage Statistics
#[derive(Debug, Clone)]
pub struct BudgetUsage {
    pub tokens_used: usize,
    pub calls_made: usize,
    pub max_tokens: usize,
    pub max_calls: usize,
}

impl BudgetUsage {
    pub fn utilization(&self) -> f32 {
        self.tokens_used as f32 / self.max_tokens as f32
    }
}
```

### 4.3 Token Consumption Flow

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                        Token Consumption Flow                               │
└─────────────────────────────────────────────────────────────────────────────┘

Before LLM Call:
┌─────────────────────────────────────────────────────────────────────────────┐
│  1. BudgetController.can_call()                                             │
│     └─ Check: calls_made < max_calls_per_query                              │
│     └─ Check: tokens_used < max_tokens_per_query                           │
│                                                                             │
│  2. BudgetController.can_call_at_level(depth)                               │
│     └─ Check: level_calls[depth] < max_calls_per_level                     │
│                                                                             │
│  3. BudgetController.estimate_cost(context)                                 │
│     └─ Estimate: input_tokens + output_tokens (reserved)                    │
│                                                                             │
│  4. BudgetController.can_afford(estimated_cost)                             │
│     └─ Check: estimated_cost <= remaining_budget                            │
│     └─ Check: estimated_cost <= max_tokens_per_call                         │
│                                                                             │
│  Decision: All pass → Continue call; Any fail → Skip or Fallback            │
└─────────────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
LLM Call:
┌─────────────────────────────────────────────────────────────────────────────┐
│  LLM Client Returns:                                                        │
│  - usage.prompt_tokens (Input tokens)                                       │
│  - usage.completion_tokens (Output tokens)                                 │
└─────────────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
After LLM Call:
┌─────────────────────────────────────────────────────────────────────────────┐
│  BudgetController.record_usage(input_tokens, output_tokens, level)          │
│  └─ tokens_used += input_tokens + output_tokens                             │
│  └─ calls_made += 1                                                         │
│  └─ level_calls[level] += 1                                                 │
│                                                                             │
│  MetricsCollector.record(...):                                              │
│  └─ total_input_tokens += input_tokens                                      │
│  └─ total_output_tokens += output_tokens                                    │
│  └─ estimated_cost = calculate_cost(tokens, model_price)                    │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## 5. Responsibility Division

### 5.1 Module Responsibilities

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                          Pilot Module Responsibilities                      │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │  QueryAnalyzer - Query Analyzer                                      │   │
│  ├─────────────────────────────────────────────────────────────────────┤   │
│  │  Responsibilities:                                                   │   │
│  │  • Analyze query complexity (Simple/Medium/Complex)                  │   │
│  │  • Extract keywords and entities                                     │   │
│  │  • Identify query intent (Fact/Compare/Explain/How-To)              │   │
│  │  • Determine if Pilot intervention is needed                         │   │
│  │                                                                      │   │
│  │  Input: query: String                                                │   │
│  │  Output: QueryAnalysis { complexity, keywords, intent, needs_pilot } │   │
│  │                                                                      │   │
│  │  Implementation Strategy:                                            │   │
│  │  • Lightweight: Rule-based (keyword count, sentence structure)       │   │
│  │  • Heavyweight: LLM analysis (complex queries)                       │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │  ContextBuilder - Context Builder                                    │   │
│  ├─────────────────────────────────────────────────────────────────────┤   │
│  │  Responsibilities:                                                   │   │
│  │  • Build context information to send to LLM                          │   │
│  │  • Extract node information (title, summary, depth) of current path  │   │
│  │  • Build descriptions of candidate nodes                             │   │
│  │  • Generate ToC view (from current node perspective)                 │   │
│  │  • Control token budget allocation                                   │   │
│  │                                                                      │   │
│  │  Input: tree, path, candidates, query                               │   │
│  │  Output: PilotContext { path_info, candidates_info, toc_view }       │   │
│  │                                                                      │   │
│  │  Token Budget Allocation:                                            │   │
│  │  • path_info: 20%                                                    │   │
│  │  • candidates_info: 50%                                              │   │
│  │  • toc_view: 30%                                                     │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │  PromptBuilder - Prompt Builder                                     │   │
│  ├─────────────────────────────────────────────────────────────────────┤   │
│  │  Responsibilities:                                                   │   │
│  │  • Select appropriate prompt template based on scenario              │   │
│  │  • Fill template variables                                           │   │
│  │  • Manage system prompt and user prompt                              │   │
│  │  • Support multiple languages                                        │   │
│  │                                                                      │   │
│  │  Scenario Types:                                                     │   │
│  │  • START: Search start, determine entry point                        │   │
│  │  • FORK: Fork in road, choose branch                                │   │
│  │  • BACKTRACK: When backtracking, analyze failure reason              │   │
│  │  • EVALUATE: Evaluate if node contains answer                        │   │
│  │                                                                      │   │
│  │  Design Points:                                                      │   │
│  │  • Configurable templates (user-customizable)                        │   │
│  │  • Include few-shot examples (improve quality)                       │   │
│  │  • Clear output format (JSON schema)                                 │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │  DecisionEngine - Decision Engine                                    │   │
│  ├─────────────────────────────────────────────────────────────────────┤   │
│  │  Responsibilities:                                                   │   │
│  │  • Determine when to call LLM (should_intervene)                     │   │
│  │  • Coordinate LLM calls                                              │   │
│  │  • Fuse algorithm scoring and LLM suggestions                        │   │
│  │  • Make final decision                                               │   │
│  │                                                                      │   │
│  │  Decision Logic:                                                     │   │
│  │  ┌─────────────────────────────────────────────────────────────┐    │   │
│  │  │  should_intervene(state) -> bool                             │    │   │
│  │  │                                                              │    │   │
│  │  │  // Strategy 1: Fork in road                                │    │   │
│  │  │  if candidates.len() > config.fork_threshold { return true } │    │   │
│  │  │                                                              │    │   │
│  │  │  // Strategy 2: Algorithm uncertain                          │    │   │
│  │  │  if scores_are_close(candidates) { return true }             │    │   │
│  │  │                                                              │    │   │
│  │  │  // Strategy 3: Low confidence                               │    │   │
│  │  │  if best_score < config.low_confidence_threshold { return true }│  │
│  │  │                                                              │    │   │
│  │  │  // Strategy 4: Budget check                                 │    │   │
│  │  │  if budget_exhausted() { return false }                      │    │   │
│  │  │                                                              │    │   │
│  │  │  return false                                                │    │   │
│  │  └─────────────────────────────────────────────────────────────┘    │   │
│  │                                                                      │   │
│  │  Fusion Logic:                                                       │   │
│  │  ┌─────────────────────────────────────────────────────────────┐    │   │
│  │  │  final_score = α * algo_score + β * llm_confidence          │    │   │
│  │  │                                                              │    │   │
│  │  │  // α and β dynamically adjust based on scenario            │    │   │
│  │  │  // - Higher β when LLM confidence is high                  │    │   │
│  │  │  // - Higher α when algorithm score is high and LLM confidence is low││
│  │  └─────────────────────────────────────────────────────────────┘    │   │
│  │                                                                      │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │  ResponseParser - Response Parser                                   │   │
│  ├─────────────────────────────────────────────────────────────────────┤   │
│  │  Responsibilities:                                                   │   │
│  │  • Parse JSON returned by LLM                                        │   │
│  │  • Handle format errors                                              │   │
│  │  • Extract structured information (ranked_candidates, direction, confidence)│
│  │  • Validate response effectiveness                                   │   │
│  │                                                                      │   │
│  │  Parsing Strategy:                                                   │   │
│  │  ┌─────────────────────────────────────────────────────────────┐    │   │
│  │  │  parse(response: String) -> Result<PilotDecision>            │    │   │
│  │  │                                                              │    │   │
│  │  │  // Priority 1: JSON parsing                                 │    │   │
│  │  │  if let Ok(json) = parse_json(response) { return json }      │    │   │
│  │  │                                                              │    │   │
│  │  │  // Priority 2: Regex extraction                             │    │   │
│  │  │  if let Some(data) = extract_by_regex(response) { return data }│   │
│  │  │                                                              │    │   │
│  │  │  // Priority 3: Default value                                │    │   │
│  │  │  return PilotDecision::default()                             │    │   │
│  │  └─────────────────────────────────────────────────────────────┘    │   │
│  │                                                                      │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
│                                                                             │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │  BudgetController - Budget Controller                               │   │
│  ├─────────────────────────────────────────────────────────────────────┤   │
│  │  Responsibilities:                                                   │   │
│  │  • Track token consumption                                          │   │
│  │  • Control LLM call frequency                                       │   │
│  │  • Estimate call cost                                               │   │
│  │  • Enforce budget limits                                            │   │
│  │                                                                      │   │
│  │  Configuration:                                                     │   │
│  │  ┌─────────────────────────────────────────────────────────────┐    │   │
│  │  │  BudgetConfig {                                             │    │   │
│  │  │    max_tokens_per_query: usize,    // Total budget per query│    │   │
│  │  │    max_tokens_per_call: usize,     // Budget per call       │    │   │
│  │  │    max_calls_per_query: usize,     // Max calls per query   │    │   │
│  │  │    max_calls_per_level: usize,     // Max calls per level   │    │   │
│  │  │    hard_limit: bool,               // Whether hard limit    │    │   │
│  │  │  }                                                          │    │   │
│  │  └─────────────────────────────────────────────────────────────┘    │   │
│  │  Interface:                                                         │   │
│  │  • can_call() -> bool                                               │   │
│  │  • can_call_at_level(level) -> bool                                 │   │
│  │  • estimate_cost(context) -> usize                                  │   │
│  │  • can_afford(estimated_cost) -> bool                               │   │
│  │  • record_usage(input, output, level)                               │   │
│  │  • remaining_budget() -> usize                                      │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │  FallbackManager - Fallback Manager                                 │   │
│  ├─────────────────────────────────────────────────────────────────────┤   │
│  │  Responsibilities:                                                   │   │
│  │  • Handle LLM call failures                                          │   │
│  │  • Provide fallback strategies                                       │   │
│  │  • Record failure reasons                                            │   │
│  │  • Automatic recovery mechanism                                      │   │
│  │                                                                      │   │
│  │  Fallback Levels:                                                    │   │
│  │  ┌─────────────────────────────────────────────────────────────┐    │   │
│  │  │  Level 0: Normal LLM call                                   │    │   │
│  │  │     ↓ Failure                                               │    │   │
│  │  │  Level 1: Retry (max 3 times, exponential backoff)         │    │   │
│  │  │     ↓ Failure                                               │    │   │
│  │  │  Level 2: Simplify prompt (reduce context)                 │    │   │
│  │  │     ↓ Failure                                               │    │   │
│  │  │  Level 3: Pure algorithm mode (complete fallback)          │    │   │
│  │  └─────────────────────────────────────────────────────────────┘    │   │
│  │                                                                      │   │
│  │  Fallback Strategies:                                                │   │
│  │  ┌─────────────────────────────────────────────────────────────┐    │   │
│  │  │  enum FallbackStrategy {                                    │    │   │
│  │  │    Retry { max_attempts: usize, backoff: BackoffPolicy },   │    │   │
│  │  │    SimplifyContext,  // Reduce context info                 │    │   │
│  │  │    UseAlgorithm,     // Use algorithm scoring               │    │   │
│  │  │    ReturnDefault,    // Return default decision             │    │   │
│  │  │  }                                                          │    │   │
│  │  └─────────────────────────────────────────────────────────────┘    │   │
│  │                                                                      │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │  PolicyManager - Policy Manager                                     │   │
│  ├─────────────────────────────────────────────────────────────────────┤   │
│  │  Responsibilities:                                                   │   │
│  │  • Manage intervention strategy configuration                       │   │
│  │  • Support multiple operation modes                                │   │
│  │  • Dynamic parameter adjustment (optional)                          │   │
│  │                                                                      │   │
│  │  Policy Modes:                                                       │   │
│  │  ┌─────────────────────────────────────────────────────────────┐    │   │
│  │  │  enum PilotMode {                                           │    │   │
│  │  │    Aggressive,   // Aggressive mode: frequent LLM calls     │    │   │
│  │  │    Balanced,     // Balanced mode: call as needed (default) │    │   │
│  │  │    Conservative, // Conservative mode: minimize LLM calls   │    │   │
│  │  │    AlgorithmOnly,// Pure algorithm mode: no LLM calls       │    │   │
│  │  │  }                                                          │    │   │
│  │  └─────────────────────────────────────────────────────────────┘    │   │
│  │                                                                      │   │
│  │  Parameter Adjustment:                                               │   │
│  │  ┌─────────────────────────────────────────────────────────────┐    │   │
│  │  │  // Dynamic adjustment based on historical performance      │    │   │
│  │  │  fn adjust_threshold(&mut self, performance: &PerformanceMetrics) {│
│  │  │    // If LLM suggestion accuracy is high, lower intervention threshold│
│  │  │    if performance.llm_accuracy > 0.8 {                      │    │   │
│  │  │      self.fork_threshold = 2;                               │    │   │
│  │  │    }                                                        │    │   │
│  │  │  }                                                          │    │   │
│  │  └─────────────────────────────────────────────────────────────┘    │   │
│  │                                                                      │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │  MetricsCollector - Metrics Collector                               │   │
│  ├─────────────────────────────────────────────────────────────────────┤   │
│  │  Responsibilities:                                                   │   │
│  │  • Collect performance metrics                                       │   │
│  │  • Track LLM call details                                            │   │
│  │  • Calculate costs                                                   │   │
│  │  • Support observability                                             │   │
│  │                                                                      │   │
│  │  Metric Types:                                                       │   │
│  │  ┌─────────────────────────────────────────────────────────────┐    │   │
│  │  │  PilotMetrics {                                             │    │   │
│  │  │    // Call statistics                                        │    │   │
│  │  │    total_calls: usize,                                      │    │   │
│  │  │    successful_calls: usize,                                 │    │   │
│  │  │    failed_calls: usize,                                     │    │   │
│  │  │    fallback_count: usize,                                   │    │   │
│  │  │                                                             │    │   │
│  │  │    // Token statistics                                       │    │   │
│  │  │    total_input_tokens: usize,                               │    │   │
│  │  │    total_output_tokens: usize,                              │    │   │
│  │  │    avg_tokens_per_call: f64,                                │    │   │
│  │  │                                                             │    │   │
│  │  │    // Latency statistics                                     │    │   │
│  │  │    total_latency_ms: u64,                                   │    │   │
│  │  │    avg_latency_ms: f64,                                     │    │   │
│  │  │    p50_latency_ms: u64,                                     │    │   │
│  │  │    p99_latency_ms: u64,                                     │    │   │
│  │  │                                                             │    │   │
│  │  │    // Effectiveness statistics (requires feedback)          │    │   │
│  │  │    llm_decision_accuracy: Option<f64>,  // LLM decision accuracy│
│  │  │    retrieval_precision: Option<f64>,     // Retrieval precision │
│  │  │  }                                                          │    │   │
│  │  └─────────────────────────────────────────────────────────────┘    │   │
│  │                                                                      │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 5.2 Pilot and Algorithm Collaboration

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                      Pilot and Algorithm Collaboration                      │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │                    Responsibility Boundaries                         │   │
│  ├─────────────────────────────────────────────────────────────────────┤   │
│  │                                                                     │   │
│  │  Pilot (Brain)                 Algorithm (Hands and Feet)           │   │
│  │  ┌─────────────────────┐        ┌─────────────────────┐            │   │
│  │  │ • Understand query intent│        │ • Execute tree traversal     │   │
│  │  │ • Analyze document structure│        │ • Efficient search path    │   │
│  │  │ • Semantic judgment   │        │ • Calculate node scores       │   │
│  │  │ • Direction decision  │        │ • Manage search state       │   │
│  │  │ • Ambiguity resolution│        │ • Return search results      │   │
│  │  └─────────────────────┘        └─────────────────────┘            │   │
│  │                                                                     │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │                    Collaboration Process                            │   │
│  ├─────────────────────────────────────────────────────────────────────┤   │
│  │                                                                     │   │
│  │  1. Algorithm executes search                                       │   │
│  │     │                                                               │   │
│  │     ▼                                                               │   │
│  │  2. Algorithm encounters decision point, asks Pilot                 │   │
│  │     │  Pilot.should_intervene(state)                                │   │
│  │     ▼                                                               │   │
│  │  3a. Pilot returns false → Algorithm continues with its own scorer  │   │
│  │     │                                                               │   │
│  │  3b. Pilot returns true → Pilot.decide(state)                       │   │
│  │     │  │                                                            │   │
│  │     │  ▼                                                            │   │
│  │     │  Pilot returns decision → Algorithm fuses decision and continues search│
│  │     │                                                               │   │
│  │     ▼                                                               │   │
│  │  4. Repeat until search completes                                   │   │
│  │                                                                     │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## 6. Complete Pilot Call Flow

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                      Complete Pilot Call Flow                               │
└─────────────────────────────────────────────────────────────────────────────┘

User Query: "How to configure max connections for PostgreSQL connection pool?"
                    │
                    ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│ Step 1: QueryAnalyzer analyzes query                                        │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  QueryAnalysis {                                                            │
│    complexity: Medium,           // Medium complexity                        │
│    keywords: ["PostgreSQL", "connection pool", "max connections", "configure"],│
│    intent: HowTo,               // How-To type                               │
│    needs_pilot: true,           // Needs Pilot intervention                  │
│  }                                                                          │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
                    │
                    ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│ Step 2: Pilot.guide_start() - Pre-search guidance                          │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  BudgetController: Check budget (pass)                                      │
│                                                                             │
│  ContextBuilder:                                                            │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │  ToC View:                                                          │   │
│  │  1. Introduction                                                    │   │
│  │  2. Installation                                                    │   │
│  │  3. Configuration                                                   │   │
│  │     3.1 Basic Config                                                │   │
│  │     3.2 Database Config                                             │   │
│  │     3.3 Advanced Config                                             │   │
│  │  4. API Reference                                                   │   │
│  │  ...                                                                │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
│  PromptBuilder: Build START scenario prompt                                 │
│                                                                             │
│  LLM Response:                                                              │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │  {                                                                   │   │
│  │    "entry_points": ["Configuration", "Database Config"],            │   │
│  │    "reasoning": "Query about database connection pool configuration, should start from Configuration chapter", │
│  │    "confidence": 0.9                                                │   │
│  │  }                                                                   │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
│  MetricsCollector: Record (input: 150, output: 50, latency: 230ms)         │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
                    │
                    ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│ Step 3: BeamSearch starts search                                           │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  Iteration 1: Root → [Introduction, Installation, Configuration, API, ...] │
│                                                                             │
│  Algorithm scoring:                                                         │
│    "Configuration" -> 0.75 (keyword match)                                 │
│    "API"          -> 0.35                                                  │
│    "Installation" -> 0.10                                                  │
│                                                                             │
│  Pilot.should_intervene():                                                  │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │  candidates.len() (6) > fork_threshold (3)  → true                  │   │
│  │  → Intervention needed                                              │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
│  Pilot.decide():                                                            │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │  LLM Analysis:                                                      │   │
│  │  "Query clearly points to configuration-related content, 'Configuration' chapter most relevant" │
│  │                                                                     │   │
│  │  ranked_candidates: [                                               │   │
│  │    ("Configuration", 0.95, "Explicitly mentions configuration"),    │   │
│  │    ("API", 0.40, "May have relevant API"),                          │   │
│  │  ]                                                                  │   │
│  │  confidence: 0.9                                                    │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
│  Fusion scoring:                                                           │
│    "Configuration" -> 0.75*0.4 + 0.95*0.6*0.9 = 0.84                      │
│                                                                             │
│  Choice: Deep dive into "Configuration" node                               │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
                    │
                    ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│ Step 4: Continue search - Iteration 2                                      │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  Current position: Root → Configuration                                    │
│  Candidates: [Basic Config, Database Config, Advanced Config, Performance Tuning] │
│                                                                             │
│  Algorithm scoring:                                                         │
│    "Database Config" -> 0.92 (strong match!)                               │
│    "Advanced Config" -> 0.45                                               │
│                                                                             │
│  Pilot.should_intervene():                                                  │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │  best_score (0.92) > low_score_threshold (0.3)  → OK               │   │
│  │  score_gap (0.47) > threshold (0.15)           → OK               │   │
│  │  → No intervention needed, algorithm is confident                 │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
│  Use algorithm score directly, choose "Database Config"                    │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
                    │
                    ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│ Step 5: Continue search - Iteration 3                                      │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  Current position: Root → Configuration → Database Config                  │
│  Candidates: [Connection String, Connection Pool, Timeout Settings, SSL Config] │
│                                                                             │
│  Algorithm scoring:                                                         │
│    "Connection Pool" -> 0.98 (perfect match!)                              │
│                                                                             │
│  → Target found, search ends                                               │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
                    │
                    ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│ Step 6: Return result                                                       │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  SearchResult {                                                             │
│    path: [Root → Configuration → Database Config → Connection Pool],        │
│    nodes_visited: 8,                                                        │
│  }                                                                          │
│                                                                             │
│  PilotMetrics {                                                             │
│    llm_calls: 2,                                                            │
│    total_tokens: 380,                                                       │
│    avg_latency: 185ms,                                                      │
│    estimated_cost: $0.0012,                                                 │
│  }                                                                          │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

## 7. Code Structure

```
src/retrieval/
├── mod.rs
├── pilot/                      # Pilot module
│   ├── mod.rs                  # Module entry
│   ├── trait.rs                # Pilot trait definition
│   ├── config.rs               # Configuration types (PilotConfig, BudgetConfig, InterventionConfig)
│   ├── decision.rs             # Decision types (PilotDecision, SearchDirection)
│   ├── analyzer.rs             # QueryAnalyzer
│   ├── builder.rs              # ContextBuilder
│   ├── engine.rs               # DecisionEngine
│   ├── parser.rs               # ResponseParser
│   ├── policy.rs               # PolicyManager
│   ├── budget.rs               # BudgetController
│   ├── fallback.rs             # FallbackManager
│   ├── metrics.rs              # MetricsCollector
│   ├── llm_pilot.rs            # LlmPilot implementation
│   ├── noop_pilot.rs           # NoopPilot implementation (empty impl, for pure algorithm mode)
│   └── prompts/                # Prompt templates
│       ├── mod.rs
│       ├── start.rs            # START scenario template
│       ├── fork.rs             # FORK scenario template
│       ├── backtrack.rs        # BACKTRACK scenario template
│       └── evaluate.rs         # EVALUATE scenario template
├── search/
│   ├── mod.rs
│   ├── trait.rs                # SearchTree trait (modified: add pilot parameter)
│   ├── scorer.rs               # NodeScorer (existing)
│   ├── beam.rs                 # BeamSearch (modified: integrate Pilot)
│   ├── greedy.rs               # GreedySearch (modified: integrate Pilot)
│   └── mcts.rs                 # MctsSearch (modified: integrate Pilot)
├── stages/
│   ├── search.rs               # SearchStage (modified: inject Pilot)
│   └── ...
└── ...
```

---

## 8. Configuration Examples

```rust
// Default configuration
let config = PilotConfig {
    mode: PilotMode::Balanced,
    budget: BudgetConfig::default(),
    intervention: InterventionConfig::default(),
    guide_at_start: true,
    guide_at_backtrack: true,
    prompt_template_path: None,
};

// High-quality mode (more LLM calls)
let high_quality_config = PilotConfig {
    mode: PilotMode::Aggressive,
    budget: BudgetConfig {
        max_tokens_per_query: 5000,
        max_tokens_per_call: 1000,
        max_calls_per_query: 10,
        max_calls_per_level: 3,
        hard_limit: false,
    },
    intervention: InterventionConfig {
        fork_threshold: 2,
        score_gap_threshold: 0.2,
        low_score_threshold: 0.4,
        max_interventions_per_level: 3,
    },
    guide_at_start: true,
    guide_at_backtrack: true,
    prompt_template_path: None,
};

// Low-cost mode (minimum LLM calls)
let low_cost_config = PilotConfig {
    mode: PilotMode::Conservative,
    budget: BudgetConfig {
        max_tokens_per_query: 500,
        max_tokens_per_call: 200,
        max_calls_per_query: 2,
        max_calls_per_level: 1,
        hard_limit: true,
    },
    intervention: InterventionConfig {
        fork_threshold: 5,
        score_gap_threshold: 0.1,
        low_score_threshold: 0.2,
        max_interventions_per_level: 1,
    },
    guide_at_start: false,
    guide_at_backtrack: true,
    prompt_template_path: None,
};

// Pure algorithm mode (no LLM calls)
let algorithm_only_config = PilotConfig {
    mode: PilotMode::AlgorithmOnly,
    ..Default::default()
};
```

---

## 9. Usage Example

```rust
use vectorless::retrieval::pilot::{LlmPilot, PilotConfig, PilotMode};
use vectorless::retrieval::search::BeamSearch;
use vectorless::llm::LlmClient;

// Create Pilot
let llm_client = LlmClient::from_env()?;
let pilot = LlmPilot::new(llm_client, PilotConfig::default());

// Create search engine (inject Pilot)
let search = BeamSearch::new().with_pilot(pilot);

// Execute search
let result = search.search(&tree, &context, &config).await?;

// View metrics
println!("LLM calls: {}", result.metrics.llm_calls);
println!("Tokens used: {}", result.metrics.tokens_used);
println!("Avg latency: {}ms", result.metrics.avg_latency_ms);
```