# Phase 2: Performance Optimization Design

## Overview

This document outlines the performance optimization strategies for vectorless v0.3.0, targeting millisecond-level response times. The optimizations are prioritized based on infrastructure readiness and expected impact.

## Priority Order

| Priority | Task | Status | Estimated Effort |
|----------|------|--------|------------------|
| 1 | Cache Strategy Optimization | **Ready** | 1 day |
| 2 | Incremental Indexing Optimization | **Ready** | 1 day |
| 3 | Parallel Retrieval Optimization | Needs baseline | 2 days |
| 4 | Memory Footprint Optimization | Needs evaluation | 2 days |

---

## 1. Cache Strategy Optimization

### Current State

The `MemoStore` is now integrated with `LlmPilot` for caching navigation decisions. However, cache hit rates can be improved through smarter caching strategies.

### Problem Statement

- Cache keys are based on exact content fingerprints
- Similar queries with slightly different phrasing cause cache misses
- No semantic similarity matching
- Cache warming is manual

### Proposed Improvements

#### 1.1 Semantic Cache Keys

Instead of exact fingerprint matching, use semantic similarity for cache lookups:

```
Current:  query_fp == cached_query_fp → hit
Proposed: similarity(query_embedding, cached_embedding) > threshold → hit
```

**Approach:**
- Pre-compute embeddings for cached queries
- Use cosine similarity or dot product for matching
- Threshold: 0.85+ similarity for cache hit
- Store top-k similar queries for approximate matching

**Benefits:**
- Higher hit rate for semantically equivalent queries
- Reduced LLM calls for similar user questions

#### 1.2 Cache Warming

Pre-populate cache with common query patterns:

**Approach:**
- Analyze historical query logs
- Identify top-N most frequent query patterns
- Pre-compute and cache Pilot decisions for common document structures
- Support configurable warm-up on engine startup

**Configuration:**
```toml
[memo]
warmup_enabled = true
warmup_top_queries = 100
warmup_on_startup = true
```

#### 1.3 Adaptive TTL

Adjust TTL based on content stability:

**Approach:**
- Static content (documentation): longer TTL (30 days)
- Dynamic content (news, logs): shorter TTL (1 day)
- Track content change frequency per document
- Adjust TTL dynamically based on change history

#### 1.4 Multi-Level Caching

Implement hierarchical caching:

```
L1: In-memory LRU (current MemoStore) - microseconds
L2: Local disk (persisted cache) - milliseconds
L3: Redis (distributed cache) - milliseconds
```

**Use Cases:**
- L1: Single-session hot data
- L2: Cross-session persistence
- L3: Multi-instance sharing

### Metrics to Track

| Metric | Current | Target |
|--------|---------|--------|
| Hit rate (repeated queries) | ~50% | **90%+** |
| Hit rate (similar queries) | 0% | **60%+** |
| Cache lookup latency | <1µs | <1µs |
| Memory per entry | ~500 bytes | ~300 bytes |

---

## 2. Incremental Indexing Optimization

### Current State

The fingerprint system (`NodeFingerprint`) is implemented and can detect subtree-level changes. However, the indexer still reprocesses entire documents on updates.

### Problem Statement

- Full document reprocessing on any change
- No partial tree updates
- Wasted LLM calls for unchanged sections

### Proposed Improvements

#### 2.1 Subtree-Level Updates

Only reprocess changed subtrees:

**Approach:**
1. Load existing document tree and fingerprints
2. Parse new document, compute new fingerprints
3. Compare `NodeFingerprint` at each level
4. Only reprocess nodes where `content_changed() == true`
5. Propagate `subtree_fp` changes upward

**Detection Logic:**
```
if node_fp.content_changed():
    → Regenerate summary for this node
if node_fp.only_descendants_changed():
    → Skip this node, process children only
if node_fp.subtree_changed():
    → Update ancestor subtree fingerprints
```

#### 2.2 Lazy Summary Regeneration

Defer summary regeneration until needed:

**Approach:**
- Mark nodes with `summary_stale = true` on content change
- Regenerate summaries lazily on first query access
- Use MemoStore to cache regenerated summaries
- Track staleness in `DocumentChangeInfo`

**Benefits:**
- Fast document updates (no immediate LLM calls)
- Spread LLM cost over time
- Better user experience for large documents

#### 2.3 Batch Processing

Process multiple changed documents efficiently:

**Approach:**
- Collect changed documents into batches
- Group similar content types together
- Use single LLM call for multiple summaries (where token budget allows)
- Implement priority queue for urgent documents

#### 2.4 Change Propagation

Optimize how changes propagate through the tree:

**Approach:**
- Use bottom-up propagation for fingerprint updates
- Only update ancestors of changed nodes
- Implement efficient diff algorithm (Myers or patience diff)
- Cache intermediate results during propagation

### Metrics to Track

| Metric | Current | Target |
|--------|---------|--------|
| Full reindex time (100KB doc) | ~5s | **<1s** |
| Incremental update (1 section) | ~5s (full) | **<100ms** |
| LLM calls per update | 10-50 | **1-5** |
| Memory during update | 2x doc size | **1.2x** |

---

## 3. Parallel Retrieval Optimization

### Current State

Retrieval is primarily sequential through the pipeline stages.

### Problem Statement

- Sequential stage execution
- No parallel candidate evaluation
- Underutilized multi-core CPUs

### Prerequisites

- [ ] Establish performance baseline with benchmarks
- [ ] Profile hot paths
- [ ] Identify parallelizable operations

### Proposed Improvements

#### 3.1 Parallel Stage Execution

Execute independent pipeline stages concurrently:

**Approach:**
- `AnalyzeStage` and initial `PlanStage` can run in parallel
- Fork-join pattern for search branches
- Use `tokio::join!` for concurrent stage execution

**Parallelization Points:**
```
┌─────────────┐
│   Analyze   │────┐
└─────────────┘    │
                   ├──▶ ┌─────────────┐ ──▶ ┌─────────────┐
┌─────────────┐    │    │   Search    │     │  Evaluate   │
│    Plan     │────┘    │  (parallel) │     │             │
└─────────────┘         └─────────────┘     └─────────────┘
```

#### 3.2 Parallel Candidate Evaluation

Evaluate multiple search candidates simultaneously:

**Approach:**
- Use `futures::stream` for concurrent evaluation
- Limit concurrency with semaphore
- Collect results with timeout
- Merge and rank results

**Concurrency Control:**
- Max concurrent evaluations: 4-8 (configurable)
- Per-evaluation timeout: 500ms
- Early termination on high-confidence result

#### 3.3 Parallel Tree Traversal

Traverse document tree branches in parallel:

**Approach:**
- Spawn tasks for each top-level branch
- Use work-stealing for load balancing
- Aggregate results with structured concurrency

### Metrics to Track

| Metric | Current | Target |
|--------|---------|--------|
| P50 retrieval latency | ~200ms | **<50ms** |
| P99 retrieval latency | ~1s | **<200ms** |
| CPU utilization | ~30% | **70%+** |
| Throughput (queries/sec) | ~5 | **20+** |

---

## 4. Memory Footprint Optimization

### Current State

Memory usage scales linearly with document size and cache capacity.

### Problem Statement

- Large documents (10MB+) can use 50MB+ memory
- Cache entries hold full strings
- No memory pressure handling

### Prerequisites

- [ ] Complete other Phase 2 optimizations
- [ ] Profile memory usage patterns
- [ ] Identify memory hot spots

### Proposed Improvements

#### 4.1 String Interning

Deduplicate common strings:

**Approach:**
- Use `string_interner` crate for titles, common phrases
- Intern node titles during parsing
- Store indices instead of full strings in hot paths

**Expected Savings:**
- 20-40% reduction in string memory
- Faster string comparisons

#### 4.2 Compressed Cache Entries

Compress cached values:

**Approach:**
- Use `zstd` or `lz4` for cache value compression
- Compress summaries and reasoning strings
- Decompress on cache hit

**Trade-offs:**
- Extra CPU for compression/decompression
- Significant memory savings for text-heavy caches

#### 4.3 Memory-Mapped Large Documents

Use mmap for large document content:

**Approach:**
- Store large documents as memory-mapped files
- Only load accessed sections into memory
- OS handles paging automatically

**Threshold:**
- Documents > 1MB: use mmap
- Documents < 1MB: load entirely

#### 4.4 Cache Eviction Under Pressure

Respond to memory pressure:

**Approach:**
- Monitor system memory usage
- Implement adaptive cache sizing
- Aggressive eviction when memory > 80% used
- Use `jemalloc` with background threads

### Metrics to Track

| Metric | Current | Target |
|--------|---------|--------|
| Memory per 1MB document | ~5MB | **<2MB** |
| Peak memory (10 docs) | ~500MB | **<200MB** |
| Cache memory efficiency | ~60% | **80%+** |
| GC pause time | N/A | **<10ms** |

---

## Implementation Timeline

```
Week 1:
├── Day 1-2: Cache Strategy Optimization
│   ├── Semantic cache keys
│   └── Adaptive TTL
├── Day 3-4: Incremental Indexing
│   ├── Subtree-level updates
│   └── Lazy summary regeneration
└── Day 5: Integration testing

Week 2:
├── Day 1-2: Performance Baseline
│   ├── Benchmark suite setup
│   └── Profiling infrastructure
├── Day 3-4: Parallel Retrieval
│   ├── Parallel stages
│   └── Concurrent evaluation
└── Day 5: Memory profiling

Week 3:
├── Day 1-2: Memory Optimization
│   ├── String interning
│   └── Compressed cache
├── Day 3-4: Final tuning
│   └── Integration testing
└── Day 5: Documentation & release prep
```

## Success Criteria

### Must Have (v0.3.0)

- [ ] 90%+ cache hit rate for repeated queries
- [ ] <1s incremental update time
- [ ] <100ms P50 retrieval latency

### Should Have

- [ ] 60%+ cache hit rate for similar queries
- [ ] 70%+ CPU utilization during retrieval
- [ ] <200MB memory for 10 documents

### Nice to Have

- [ ] Multi-level caching (L1/L2/L3)
- [ ] Memory-mapped document storage
- [ ] Distributed cache support

## Dependencies

| Optimization | Requires |
|-------------|----------|
| Semantic cache keys | Embedding model (local or API) |
| Parallel retrieval | `tokio` profiling tools |
| Memory optimization | Memory profiler (`dhall` or `bytehound`) |

## Risks

| Risk | Mitigation |
|------|------------|
| Semantic cache adds latency | Use local embedding model (all-MiniLM) |
| Parallel execution complexity | Extensive testing, structured concurrency |
| Memory optimization regressions | Benchmark before/after each change |
| Cache coherence issues | Clear invalidation strategy, versioning |

## References

- [MemoStore Design](./memo.md)
- [Fingerprint System](./fingerprint.md)
- [Incremental Indexing](./incremental.md)
- [Pilot Architecture](./pilot.md)
