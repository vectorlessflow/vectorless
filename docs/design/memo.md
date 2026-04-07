# LLM Memoization System

## Overview

The memoization system provides intelligent caching for expensive LLM operations, reducing API costs and latency while maintaining semantic correctness.

## Architecture

```
┌─────────────────────────────────────────────────────────────────────┐
│                          Memoization Layer                          │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  ┌──────────────┐    ┌──────────────┐    ┌──────────────┐         │
│  │    Engine    │───▶│  Retriever   │───▶│   LlmPilot   │         │
│  │   Builder    │    │   Pipeline   │    │              │         │
│  └──────────────┘    └──────────────┘    └──────────────┘         │
│         │                   │                   │                  │
│         └───────────────────┴───────────────────┘                  │
│                             │                                      │
│                    ┌────────▼────────┐                            │
│                    │   MemoStore     │                            │
│                    │                 │                            │
│                    │  ┌───────────┐  │                            │
│                    │  │ LRU Cache │  │                            │
│                    │  └───────────┘  │                            │
│                    │  ┌───────────┐  │                            │
│                    │  │   Stats   │  │                            │
│                    │  └───────────┘  │                            │
│                    │  ┌───────────┐  │                            │
│                    │  │   TTL     │  │                            │
│                    │  └───────────┘  │                            │
│                    └─────────────────┘                            │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

## Key Components

### MemoKey

Content-addressed cache key that ensures cache hits only occur when inputs are semantically identical.

```rust
pub struct MemoKey {
    /// Type of operation (Summary, PilotDecision, QueryAnalysis, etc.)
    pub op_type: MemoOpType,

    /// Fingerprint of the input content (BLAKE2b-128)
    pub input_fp: Fingerprint,

    /// Model identifier for cache invalidation when model changes
    pub model_id: Option<String>,

    /// Version for cache invalidation when algorithm changes
    pub version: u32,

    /// Additional context fingerprint (e.g., navigation context for pilot)
    pub context_fp: Fingerprint,
}
```

### MemoStore

Thread-safe LRU cache with TTL expiration and optional disk persistence.

```rust
pub struct MemoStore {
    cache: Arc<RwLock<LruCache<String, MemoEntry>>>,
    stats: Arc<AsyncRwLock<MemoStats>>,
    ttl: Duration,
    model_id: Option<String>,
    version: u32,
}
```

**Features:**
- LRU eviction policy (default: 10,000 entries)
- TTL-based expiration (default: 7 days)
- Optional disk persistence (JSON format)
- Thread-safe access via `parking_lot::RwLock`

### Integration Points

| Component | Operation Type | Description |
|-----------|---------------|-------------|
| `LlmSummaryGenerator` | `Summary` | Node summary generation |
| `LlmPilot` | `PilotDecision` | Navigation decision caching |
| Query Analyzer | `QueryAnalysis` | Query complexity/intent analysis |
| Content Extractor | `Extraction` | Structured data extraction |

## Design Principles

### 1. Layered Architecture

Each layer can be independently configured and tested:

```
Engine → PipelineRetriever → LlmPilot → MemoStore
```

Benefits:
- `MemoStore` can be reused by multiple components
- Each layer has single responsibility
- Easy to mock for testing

### 2. Non-Intrusive Integration

Memoization is optional and doesn't break existing APIs:

```rust
// Without memoization (works as before)
let pilot = LlmPilot::new(client, config);

// With memoization (opt-in)
let pilot = LlmPilot::new(client, config)
    .with_memo_store(store);
```

### 3. Smart Cache Key Design

Cache keys include semantic context for precise invalidation:

```rust
// Key automatically invalidates when:
// - Model changes (model_id field)
// - Algorithm version changes (version field)
// - Input content changes (input_fp field)
// - Navigation context changes (context_fp field)
```

### 4. Cost Tracking

The system tracks savings to quantify the value of caching:

```rust
pub struct MemoStats {
    pub entries: usize,
    pub hits: u64,
    pub misses: u64,
    pub tokens_saved: u64,
    pub cost_saved: f64,
}

impl MemoStats {
    pub fn hit_rate(&self) -> f64 {
        let total = self.hits + self.misses;
        if total == 0 { 0.0 } else { self.hits as f64 / total as f64 }
    }
}
```

### 5. Flexible Invalidation Strategies

```rust
// Time-based (automatic)
store.with_ttl(Duration::days(7))

// By operation type
store.invalidate_by_op_type(MemoOpType::PilotDecision)

// By model prefix
store.invalidate_by_model_prefix("gpt-4")

// Manual
store.remove(&key)
store.clear()
```

## Usage Examples

### Basic Setup

```rust
use vectorless::memo::MemoStore;
use chrono::Duration;

// Create with custom settings
let store = MemoStore::new()
    .with_ttl(Duration::days(7))
    .with_model("gpt-4o")
    .with_version(1);
```

### With Engine Builder

```rust
use vectorless::client::EngineBuilder;

// Option 1: Custom memo store
let memo_store = MemoStore::new()
    .with_ttl(Duration::days(7))
    .with_model("gpt-4o");

let engine = EngineBuilder::new()
    .with_workspace("./data")
    .with_memo_store(memo_store)
    .with_openai(api_key)
    .build()
    .await?;

// Option 2: Default (auto-created with config model)
let engine = EngineBuilder::new()
    .with_workspace("./data")
    .with_openai(api_key)
    .build()
    .await?;
```

### Monitoring Cache Performance

```rust
// Async stats (includes all metrics)
let stats = store.stats().await;
println!("Hit rate: {:.2}%", stats.hit_rate() * 100.0);
println!("Tokens saved: {}", stats.tokens_saved);

// Sync snapshot (for monitoring without async)
let stats = store.stats_snapshot();
println!("Cache entries: {}", stats.entries);
```

### Cache Invalidation

```rust
// When switching models
store.invalidate_by_model_prefix("gpt-3.5");

// When algorithm changes
store.invalidate_by_op_type(MemoOpType::PilotDecision);

// Manual pruning of expired entries
let removed = store.prune_expired();
```

### Persistence

```rust
// Save to disk
store.save(Path::new("./cache/memo.json")).await?;

// Load from disk (on startup)
store.load(Path::new("./cache/memo.json")).await?;
```

## Performance Characteristics

### Concurrency

| Component | Lock Type | Rationale |
|-----------|-----------|-----------|
| LRU Cache | `parking_lot::RwLock` | High-performance, allows concurrent reads |
| Statistics | `tokio::sync::RwLock` | Async-compatible for integration |
| Atomic Stats | `AtomicU64` | Lock-free for hot paths |

### Memory

- Default capacity: 10,000 entries
- Per-entry overhead: ~200-500 bytes (depending on cached value size)
- Estimated memory: 2-5 MB at full capacity

### Latency

| Operation | Typical Latency |
|-----------|-----------------|
| Cache hit | < 1 µs |
| Cache miss (no compute) | < 5 µs |
| Cache miss (with LLM) | 100-2000 ms |

## Cost Savings Estimation

### Typical Document Retrieval Scenario

| Scenario | Without Cache | With Cache | Savings |
|----------|---------------|------------|---------|
| First query | 5-10 LLM calls | 5-10 LLM calls | 0% |
| Repeated query | 5-10 LLM calls | 0-1 LLM calls | **80-100%** |
| Similar query | 5-10 LLM calls | 2-3 LLM calls | **50-70%** |

### Token Savings Example

```rust
// Assuming GPT-4 pricing: $0.03 / 1K input tokens, $0.06 / 1K output tokens
// Average Pilot decision: 500 input tokens, 100 output tokens

// Without cache (100 queries):
// Cost = 100 * (500 * 0.03/1000 + 100 * 0.06/1000) = $2.10

// With 80% hit rate:
// Cost = 20 * $0.021 = $0.42
// Savings = $1.68 (80%)
```

## Future Improvements

### Potential Enhancements

1. **Semantic Cache Keys**: Use embedding similarity for fuzzy matching
2. **Distributed Cache**: Share cache across multiple instances via Redis
3. **Compression**: Compress cached values for large responses
4. **Warm-up**: Pre-populate cache with common patterns
5. **Analytics Dashboard**: Real-time visualization of cache performance

### Implementation Notes

- Consider using `AtomicU64` for all stats to eliminate async lock overhead
- Cache `MemoKey::fingerprint()` result for frequently used keys
- Add automatic periodic persistence with configurable interval

## Related Documentation

- [Fingerprint System](./fingerprint.md) - Content-addressed hashing
- [Incremental Indexing](./incremental.md) - Change detection for reindexing
- [Pilot Architecture](./pilot.md) - LLM-based navigation intelligence
