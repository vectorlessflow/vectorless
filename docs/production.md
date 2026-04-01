# Vectorless Production-Ready Architecture Proposal

## Executive Summary

This document outlines the necessary improvements to transform Vectorless from a prototype into a production-ready document intelligence engine. 

---

## 1. Current Architecture Gap Analysis

1. **No TOC Processing Pipeline**: sophisticated TOC detection, extraction, and validation
2. **No Verification Mechanism**: Cannot detect or repair indexing errors
3. **No Concurrency Control**: Will overwhelm LLM APIs under load
4. **No Graceful Degradation**: Single failure breaks entire pipeline

---

## 2. Core Architecture Improvements

### 2.1 Concurrency Control Layer (Critical)

The current implementation has no rate limiting or concurrency control, which will cause:
- API rate limit errors
- Resource exhaustion
- Unpredictable latency

**Proposed Implementation:**

```rust
// src/concurrency/mod.rs
use std::sync::Arc;
use tokio::sync::Semaphore;

pub struct ConcurrencyConfig {
    /// Maximum concurrent LLM API calls
    pub max_concurrent_llm_calls: usize,
    /// Maximum concurrent I/O operations
    pub max_concurrent_io_ops: usize,
    /// API rate limit (requests per minute)
    pub rate_limit_per_minute: usize,
    /// Retry strategy for transient failures
    pub retry_strategy: RetryStrategy,
}

pub struct ConcurrencyController {
    llm_semaphore: Arc<Semaphore>,
    io_semaphore: Arc<Semaphore>,
    rate_limiter: Arc<RateLimiter>,
}

pub struct RateLimiter {
    // Using governor for token bucket algorithm
    inner: governor::RateLimiter<
        governor::state::InMemoryState,
        governor::clock::DefaultClock,
    >,
}

impl ConcurrencyController {
    pub fn new(config: ConcurrencyConfig) -> Self {
        Self {
            llm_semaphore: Arc::new(Semaphore::new(config.max_concurrent_llm_calls)),
            io_semaphore: Arc::new(Semaphore::new(config.max_concurrent_io_ops)),
            rate_limiter: Arc::new(RateLimiter::new(config.rate_limit_per_minute)),
        }
    }

    pub async fn acquire_llm_permit(&self) -> SemaphorePermit<'_> {
        self.rate_limiter.wait().await;
        self.llm_semaphore.acquire().await.unwrap()
    }
}
```

**Usage in Vectorless client:**

```rust
impl Vectorless {
    pub async fn generate_summaries(&self, tree: &mut DocumentTree) -> Result<()> {
        let nodes = tree.collect_nodes();
        
        let results = futures::stream::iter(nodes)
            .map(|node_id| async {
                let _permit = self.concurrency.acquire_llm_permit().await;
                self.generate_single_summary(node_id).await
            })
            .buffer_unordered(self.config.concurrency.max_concurrent_llm_calls)
            .try_collect::<Vec<_>>()
            .await?;
        
        Ok(())
    }
}
```

### 2.2 Verification-Repair Loop (Critical)

self-correcting pipeline:

```
Index → Verify → [Errors?] → Repair → Verify → ...
                         ↓
                    [No Errors] → Done
```

**Proposed Implementation:**

```rust
// src/verification/mod.rs

pub struct VerificationConfig {
    /// Number of nodes to sample for verification (None = all)
    pub sample_size: Option<usize>,
    /// Minimum accuracy threshold (0.0 - 1.0)
    pub accuracy_threshold: f32,
    /// Maximum repair attempts before fallback
    pub max_fix_attempts: usize,
    /// Maximum concurrent repair operations
    pub max_concurrent_repairs: usize,
}

impl Default for VerificationConfig {
    fn default() -> Self {
        Self {
            sample_size: None,
            accuracy_threshold: 0.6,
            max_fix_attempts: 3,
            max_concurrent_repairs: 5,
        }
    }
}

pub struct VerificationReport {
    pub accuracy: f32,
    pub total_checked: usize,
    pub errors: Vec<VerificationError>,
    pub repairs_applied: usize,
    pub fallback_used: bool,
}

pub async fn verify_and_repair(
    tree: &mut DocumentTree,
    page_list: &[PageContent],
    config: &VerificationConfig,
    llm_client: &LlmClient,
) -> Result<VerificationReport> {
    let mut attempts = 0;
    let mut total_repairs = 0;
    
    while attempts < config.max_fix_attempts {
        // 1. Sample and verify nodes
        let sample = select_sample(tree, config.sample_size);
        let (accuracy, errors) = verify_sample(&sample, page_list, llm_client).await?;
        
        if accuracy >= config.accuracy_threshold {
            return Ok(VerificationReport {
                accuracy,
                total_checked: sample.len(),
                errors: vec![],
                repairs_applied: total_repairs,
                fallback_used: false,
            });
        }
        
        // 2. Concurrently repair errors
        let repairs = futures::stream::iter(errors.clone())
            .map(|error| repair_single_node(error, page_list, llm_client))
            .buffer_unordered(config.max_concurrent_repairs);
        
        let repair_results: Vec<_> = repairs.try_collect().await?;
        
        // 3. Apply successful repairs
        for (node_id, correction) in repair_results.into_iter().flatten() {
            tree.update_node_page_index(node_id, correction);
            total_repairs += 1;
        }
        
        attempts += 1;
    }
    
    // 4. Fallback: degrade processing mode
    Ok(VerificationReport {
        accuracy: 0.0,
        total_checked: 0,
        errors: vec![],
        repairs_applied: total_repairs,
        fallback_used: true,
    })
}

async fn verify_sample(
    nodes: &[NodeId],
    page_list: &[PageContent],
    llm_client: &LlmClient,
) -> Result<(f32, Vec<VerificationError>)> {
    let results: Vec<_> = futures::stream::iter(nodes)
        .map(|node_id| verify_node_appearance(*node_id, page_list, llm_client))
        .buffer_unordered(10)
        .try_collect()
        .await?;
    
    let correct = results.iter().filter(|r| r.is_correct).count();
    let accuracy = correct as f32 / results.len() as f32;
    
    let errors = results.into_iter()
        .filter(|r| !r.is_correct)
        .map(|r| r.error)
        .collect();
    
    Ok((accuracy, errors))
}
```

### 2.3 Multi-Mode Processing Pipeline (Critical)

supports three processing modes based on document characteristics:

**Proposed Implementation:**

```rust
// src/processor/mod.rs

pub enum ProcessingMode {
    /// Document has TOC with page numbers
    TocWithPageNumbers,
    /// Document has TOC without page numbers
    TocWithoutPageNumbers,
    /// Document has no TOC, structure must be extracted
    NoToc,
}

pub struct ProcessingResult {
    pub tree: DocumentTree,
    pub mode_used: ProcessingMode,
    pub verification: VerificationReport,
    pub metadata: DocumentMetadata,
}

pub struct DocumentProcessor {
    toc_detector: TocDetector,
    toc_extractor: TocExtractor,
    page_assigner: Assigner,
    structure_extractor: StructureExtractor,
    config: ProcessingConfig,
}

impl DocumentProcessor {
    pub async fn process(&self, pages: &[Page]) -> Result<ProcessingResult> {
        // 1. Detect TOC presence
        let toc_result = self.toc_detector.detect(pages).await?;
        
        // 2. Select appropriate processing mode
        let mode = self.select_mode(&toc_result);
        tracing::info!("Selected processing mode: {:?}", mode);
        
        // 3. Execute mode-specific pipeline
        let tree = match mode {
            ProcessingMode::TocWithPageNumbers => {
                self.process_toc_with_numbers(&toc_result, pages).await?
            }
            ProcessingMode::TocWithoutPageNumbers => {
                self.process_toc_no_numbers(&toc_result, pages).await?
            }
            ProcessingMode::NoToc => {
                self.process_no_toc(pages).await?
            }
        };
        
        // 4. Verify and repair
        let verification = verify_and_repair(
            &mut tree,
            &pages,
            &self.config.verification,
            &self.config.llm_client,
        ).await?;
        
        // 5. Fallback if verification failed
        if verification.fallback_used {
            return self.process_with_fallback(pages, mode).await;
        }
        
        Ok(ProcessingResult {
            tree,
            mode_used: mode,
            verification,
            metadata: self.extract_metadata(pages),
        })
    }
    
    fn select_mode(&self, toc_result: &TocDetectionResult) -> ProcessingMode {
        match toc_result {
            TocDetectionResult::FoundWithPageNumbers { .. } => {
                ProcessingMode::TocWithPageNumbers
            }
            TocDetectionResult::FoundWithoutPageNumbers { .. } => {
                ProcessingMode::TocWithoutPageNumbers
            }
            TocDetectionResult::NotFound => ProcessingMode::NoToc,
        }
    }
}
```

### 2.4 Storage Abstraction Layer (Important)

Current JSON file storage won't scale beyond 10K documents.

**Proposed Implementation:**

```rust
// src/storage/backend.rs

#[async_trait]
pub trait StorageBackend: Send + Sync {
    async fn get(&self, id: &str) -> Result<Option<Vec<u8>>>;
    async fn put(&self, id: &str, data: &[u8]) -> Result<()>;
    async fn delete(&self, id: &str) -> Result<bool>;
    async fn list(&self) -> Result<Vec<String>>;
    async fn exists(&self, id: &str) -> Result<bool>;
    
    // Batch operations for efficiency
    async fn get_batch(&self, ids: &[&str]) -> Result<Vec<Option<Vec<u8>>>>;
    async fn put_batch(&self, items: &[(&str, &[u8])]) -> Result<()>;
}

// JSON file implementation (current, for small deployments)
pub struct JsonFileBackend {
    root: PathBuf,
    cache: LruCache<String, Vec<u8>>,
}

// Sled implementation (recommended for >10K documents)
#[cfg(feature = "sled-storage")]
pub struct SledBackend {
    db: sled::Db,
}

#[cfg(feature = "sled-storage")]
#[async_trait]
impl StorageBackend for SledBackend {
    async fn get(&self, id: &str) -> Result<Option<Vec<u8>>> {
        Ok(self.db.get(id)?.map(|ivec| ivec.to_vec()))
    }
    
    async fn put(&self, id: &str, data: &[u8]) -> Result<()> {
        self.db.insert(id, data)?;
        self.db.flush_async().await?;
        Ok(())
    }
    
    // ... other methods
}

// Cloud storage implementation (S3, Azure, GCS)
#[cfg(feature = "cloud-storage")]
pub struct CloudStorageBackend {
    client: object_store::ObjectStore,
    prefix: String,
}

// Configuration
pub struct StorageConfig {
    pub backend: StorageType,
    pub cache_size: usize,
    pub compression: CompressionConfig,
    pub encryption: Option<EncryptionConfig>,
}

pub enum StorageType {
    JsonFile { path: PathBuf },
    Sled { path: PathBuf },
    S3 { bucket: String, prefix: String },
    Azure { container: String, prefix: String },
}
```

### 2.5 Large Document Chunked Processing (Important)

For documents exceeding token limits, implement overlapping chunk processing:

```rust
// src/indexer/chunked_processor.rs

pub struct ChunkedProcessor {
    max_pages_per_chunk: usize,
    max_tokens_per_chunk: usize,
    overlap_pages: usize,
    concurrency: ConcurrencyController,
}

impl ChunkedProcessor {
    pub async fn process_large_document(
        &self,
        pages: &[Page],
    ) -> Result<DocumentTree> {
        // 1. Create overlapping chunks
        let chunks = self.create_overlapping_chunks(pages);
        tracing::info!("Split document into {} chunks", chunks.len());
        
        // 2. Process chunks concurrently
        let partial_trees = futures::stream::iter(&chunks)
            .map(|chunk| self.process_chunk(chunk))
            .buffer_unordered(self.concurrency.max_concurrent_chunks)
            .try_collect::<Vec<_>>()
            .await?;
        
        // 3. Merge partial trees
        self.merge_partial_trees(partial_trees, &chunks)
    }
    
    fn create_overlapping_chunks(&self, pages: &[Page]) -> Vec<Chunk> {
        let mut chunks = Vec::new();
        let mut start = 0;
        
        while start < pages.len() {
            let end = (start + self.max_pages_per_chunk).min(pages.len());
            let token_count: usize = pages[start..end]
                .iter()
                .map(|p| p.token_count)
                .sum();
            
            chunks.push(Chunk {
                pages: &pages[start..end],
                start_index: start,
                end_index: end,
                token_count,
            });
            
            // Move forward with overlap
            start = end.saturating_sub(self.overlap_pages);
            
            if end >= pages.len() {
                break;
            }
        }
        
        chunks
    }
    
    /// Recursively process large nodes
    pub async fn process_large_node_recursively(
        &self,
        node: &mut TreeNode,
        pages: &[Page],
    ) -> Result<()> {
        // Check if node needs subdivision
        if self.should_subdivide(node, pages) {
            let node_pages = &pages[node.start_page..node.end_page];
            
            // Recursively process
            let sub_tree = self.process_large_document(node_pages).await?;
            
            // Attach children to current node
            node.children = sub_tree.children;
        }
        
        // Process all children recursively
        futures::future::try_join_all(
            node.children.iter_mut()
                .map(|child| self.process_large_node_recursively(child, pages))
        ).await
    }
    
    fn should_subdivide(&self, node: &TreeNode, pages: &[Page]) -> bool {
        let page_count = node.end_page - node.start_page;
        let token_count: usize = pages[node.start_page..node.end_page]
            .iter()
            .map(|p| p.token_count)
            .sum();
        
        page_count > self.max_pages_per_chunk 
            && token_count > self.max_tokens_per_chunk
    }
}
```

### 2.6 Observability (Important)

Production systems require comprehensive observability:

```rust
// src/observability/mod.rs

use metrics::{counter, histogram, gauge};
use tracing::{info_span, instrument};

pub struct Metrics {
    // Counters
    pub documents_indexed: Counter,
    pub documents_failed: Counter,
    pub llm_calls_total: Counter,
    pub llm_errors: Counter,
    
    // Histograms
    pub llm_latency: Histogram,
    pub indexing_latency: Histogram,
    pub retrieval_latency: Histogram,
    pub document_size: Histogram,
    
    // Gauges
    pub cache_size: Gauge,
    pub active_requests: Gauge,
}

impl Metrics {
    pub fn new() -> Self {
        Self {
            documents_indexed: counter!("vectorless_documents_indexed_total"),
            documents_failed: counter!("vectorless_documents_failed_total"),
            llm_calls_total: counter!("vectorless_llm_calls_total"),
            llm_errors: counter!("vectorless_llm_errors_total"),
            llm_latency: histogram!("vectorless_llm_latency_seconds"),
            indexing_latency: histogram!("vectorless_indexing_latency_seconds"),
            retrieval_latency: histogram!("vectorless_retrieval_latency_seconds"),
            document_size: histogram!("vectorless_document_size_bytes"),
            cache_size: gauge!("vectorless_cache_size"),
            active_requests: gauge!("vectorless_active_requests"),
        }
    }
}

// Tracing spans for distributed tracing
impl Vectorless {
    #[instrument(skip_all, fields(doc_path = %path.display()))]
    pub async fn index(&mut self, path: impl AsRef<Path>) -> Result<String> {
        let span = info_span!("index_document");
        let _enter = span.enter();
        
        // ... implementation with nested spans
    }
}
```

**Configuration:**

```rust
pub struct ObservabilityConfig {
    pub metrics_exporter: MetricsExporter,
    pub trace_exporter: TraceExporter,
    pub log_level: LevelFilter,
}

pub enum MetricsExporter {
    Prometheus { endpoint: SocketAddr },
    Otlp { endpoint: String },
    None,
}

pub enum TraceExporter {
    Jaeger { agent_endpoint: String },
    Otlp { endpoint: String },
    None,
}
```

### 2.7 Error Handling and Graceful Degradation (Important)

```rust
// src/error/recovery.rs

pub enum RecoveryStrategy {
    /// Retry with exponential backoff
    Retry {
        max_attempts: usize,
        backoff: BackoffConfig,
    },
    /// Fall back to alternative strategy
    Fallback {
        alternative: Box<RecoveryStrategy>,
    },
    /// Skip and continue with warning
    Skip {
        log_level: Level,
    },
    /// Fail immediately
    Fail,
}

pub struct BackoffConfig {
    pub initial_delay: Duration,
    pub max_delay: Duration,
    pub multiplier: f64,
}

impl Vectorless {
    pub async fn index_with_recovery(
        &mut self,
        path: &Path,
        strategy: &RecoveryStrategy,
    ) -> Result<String> {
        match strategy {
            RecoveryStrategy::Retry { max_attempts, backoff } => {
                let mut attempts = 0;
                loop {
                    match self.index(path).await {
                        Ok(id) => return Ok(id),
                        Err(e) if e.is_retryable() && attempts < *max_attempts => {
                            attempts += 1;
                            let delay = backoff.delay(attempts);
                            tracing::warn!(
                                attempt = attempts,
                                delay_ms = delay.as_millis(),
                                error = %e,
                                "Retrying index operation"
                            );
                            tokio::time::sleep(delay).await;
                        }
                        Err(e) => return Err(e),
                    }
                }
            }
            RecoveryStrategy::Fallback { alternative } => {
                match self.index(path).await {
                    Ok(id) => Ok(id),
                    Err(_) => self.index_with_recovery(path, alternative).await,
                }
            }
            RecoveryStrategy::Skip { log_level } => {
                match self.index(path).await {
                    Ok(id) => Ok(id),
                    Err(e) => {
                        tracing::log!(log_level, "Skipping document: {}", e);
                        Err(e)
                    }
                }
            }
            RecoveryStrategy::Fail => self.index(path).await,
        }
    }
}
```

---

## 3. Proposed Project Structure

```
src/
├── lib.rs
├── error.rs
│
├── concurrency/              # NEW: Concurrency control
│   ├── mod.rs
│   ├── rate_limiter.rs       # Token bucket rate limiting
│   ├── semaphore.rs          # Concurrent request limiting
│   └── retry.rs              # Retry with backoff
│
├── processor/                # NEW: Multi-mode processing
│   ├── mod.rs
│   ├── toc_detector.rs       # TOC detection
│   ├── toc_extractor.rs      # TOC extraction
│   ├── page_assigner.rs      # Page index assignment
│   ├── structure_extractor.rs # Structure extraction for no-TOC
│   └── modes.rs              # Processing mode implementations
│
├── verification/             # NEW: Verification-repair loop
│   ├── mod.rs
│   ├── verifier.rs           # Node verification
│   ├── repairer.rs           # Error repair
│   └── report.rs             # Verification reports
│
├── storage/
│   ├── mod.rs
│   ├── backend.rs            # NEW: StorageBackend trait
│   ├── json_backend.rs       # Refactored current implementation
│   ├── sled_backend.rs       # NEW: Sled for large deployments
│   ├── cloud_backend.rs      # NEW: S3/Azure/GCS
│   └── cache.rs
│
├── observability/            # NEW: Metrics and tracing
│   ├── mod.rs
│   ├── metrics.rs            # Prometheus/OTLP metrics
│   └── tracing.rs            # Distributed tracing
│
├── core/                     # Existing
├── document/                 # Existing
├── indexer/                  # Existing (add chunked_processor.rs)
├── summarizer/               # Existing
├── retriever/                # Existing
├── ranking/                  # Existing
├── config/                   # Existing
├── registry/                 # Existing
├── client/                   # Existing
└── utils/                    # Existing
```

---

## 4. Dependency Updates

```toml
[package]
name = "vectorless"
version = "0.2.0"
edition = "2024"

[dependencies]
# Async runtime
tokio = { version = "1", features = ["full", "tracing"] }
async-trait = "0.1"
futures = "0.3"

# Serialization
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
toml = "0.8"

# Error handling
thiserror = "2"
anyhow = "1.0"  # For application-level error handling

# Rate limiting
governor = "0.6"

# Retry with backoff
backoff = "0.4"

# LLM client
async-openai = { version = "0.34", features = ["chat-completion"] }

# Observability
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter", "json"] }
metrics = "0.22"
metrics-exporter-prometheus = "0.14"

# Utilities
uuid = { version = "1.10", features = ["v4", "serde"] }
chrono = { version = "0.4", default-features = false, features = ["serde", "clock"] }
regex = "1.10"
indextree = { version = "4.8.0", features = ["deser"] }
lru = "0.12"

# Optional storage backends
sled = { version = "0.34", optional = true }
object_store = { version = "0.10", features = ["aws", "azure"], optional = true }

# PDF processing
pdf-extract = "0.7"
lopdf = "0.33"

[dev-dependencies]
tempfile = "3.10"
criterion = "0.5"
tokio-test = "0.4"

[features]
default = ["json-storage"]
json-storage = []
sled-storage = ["sled"]
cloud-storage = ["object_store"]
```

---

## 5. Implementation Roadmap

### Phase 1: Critical Path (2-3 weeks)

| Priority | Task | Estimated Effort | Impact |
|----------|------|------------------|--------|
| P0 | Concurrency control layer | 3-5 days | Production-required |
| P0 | PDF complete implementation | 5-7 days | Core functionality |
| P0 | Verification-repair loop | 3-4 days | Quality assurance |

### Phase 2: Stability (1-2 weeks)

| Priority | Task | Estimated Effort | Impact |
|----------|------|------------------|--------|
| P1 | Error recovery mechanism | 2-3 days | Stability |
| P1 | Chunked processing | 2-3 days | Large document support |
| P1 | TOC detection/extraction | 3-4 days | Processing accuracy |

### Phase 3: Scalability (1-2 weeks)

| Priority | Task | Estimated Effort | Impact |
|----------|------|------------------|--------|
| P2 | Storage abstraction layer | 2-3 days | Extensibility |
| P2 | Sled backend | 1-2 days | Large-scale deployment |
| P2 | Observability | 2 days | Operations |

### Phase 4: Cloud Ready (1 week)

| Priority | Task | Estimated Effort | Impact |
|----------|------|------------------|--------|
| P3 | Cloud storage backend | 2-3 days | Distributed deployment |
| P3 | Distributed tracing | 1-2 days | Debugging |

---

## 6. Feature Parity Checklist

### TOC Processing
- [ ] `check_toc` - TOC presence detection
- [ ] `find_toc_pages` - Locate TOC pages
- [ ] `toc_transformer` - Convert raw TOC to structured format
- [ ] `toc_extractor` - Extract TOC content
- [ ] `toc_index_extractor` - Extract page indices from TOC

### Page Index Assignment
- [ ] `calculate_page_offset` - Calculate offset between TOC and actual pages
- [ ] `add_page_offset_to_toc_json` - Apply offset corrections
- [ ] `process_none_page_numbers` - Handle missing page numbers
- [ ] `add_page_number_to_toc` - LLM-based page number assignment

### Verification
- [ ] `verify_toc` - Sample-based verification
- [ ] `check_title_appearance` - Verify section appears on page
- [ ] `check_title_appearance_in_start` - Verify section starts on page

### Repair
- [ ] `fix_incorrect_toc` - Repair incorrect indices
- [ ] `single_toc_item_index_fixer` - Fix single item
- [ ] `fix_incorrect_toc_with_retries` - Retry loop

### Large Document Handling
- [ ] `page_list_to_group_text` - Chunk pages for processing
- [ ] `process_large_node_recursively` - Recursive subdivision

### Additional Features
- [ ] `generate_doc_description` - Document-level description
- [ ] `validate_and_truncate_physical_indices` - Bounds validation
- [ ] `write_node_id` - Node ID assignment
- [ ] `add_node_text` - Node text extraction
- [ ] Concurrent LLM calls (`asyncio.gather` equivalent)

---

## 7. Configuration Example

```toml
# config.toml

[concurrency]
max_concurrent_llm_calls = 10
max_concurrent_io_ops = 20
rate_limit_per_minute = 1000

[concurrency.retry]
max_attempts = 3
initial_delay_ms = 100
max_delay_ms = 10000
multiplier = 2.0

[verification]
sample_size = null  # null = all nodes
accuracy_threshold = 0.6
max_fix_attempts = 3
max_concurrent_repairs = 5

[processor]
toc_check_page_num = 20
max_page_num_each_node = 50
max_token_num_each_node = 20000

[summary]
model = "gpt-4o-mini"
endpoint = "https://api.openai.com/v1"
api_key = "${OPENAI_API_KEY}"

[retrieval]
model = "gpt-4o"
retriever_type = "llm_navigate"
top_k = 3

[storage]
backend = "sled"  # "json", "sled", "s3"
workspace_dir = "./workspace"
cache_size = 1000
compression = true

[observability]
metrics_enabled = true
metrics_endpoint = "0.0.0.0:9090"
tracing_enabled = true
log_level = "info"
```

---

## 8. Summary

Transforming Vectorless into a production-ready system requires:

1. **Concurrency Control**: Prevent API overload and resource exhaustion
2. **Verification-Repair Loop**: Self-correcting pipeline for accuracy
3. **Multi-Mode Processing**: Handle diverse document types
4. **Storage Abstraction**: Scale beyond JSON files
5. **Observability**: Production monitoring and debugging
6. **Error Recovery**: Graceful degradation under failure

