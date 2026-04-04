# Client Module Refactoring Design

## Overview

This document describes the refactoring of the `client` module to achieve a more professional, product-level architecture with clear separation of concerns.

## Current Problems

### 1. God Object Anti-pattern
`engine.rs` (600+ lines) handles too many responsibilities:
- Document indexing
- Document retrieval
- Workspace management
- Configuration management
- Format detection
- Page parsing

### 2. Mixed Abstraction Levels
High-level operations (`query()`) mixed with low-level utilities (`parse_page_range()`).

### 3. No Session Management
Each operation is independent; no way to maintain context across multiple operations.

### 4. Missing Event System
No progress callbacks or event hooks for long-running operations.

### 5. Scattered State Management
State split across `Arc<RwLock<Workspace>>`, `Arc<Mutex<Executor>>`, `Arc<Retriever>`.

---

## Proposed Architecture

### Module Structure

```
src/client/
├── mod.rs           # Re-exports and documentation
├── engine.rs        # Core orchestrator (simplified)
├── builder.rs       # Builder pattern (enhanced)
├── types.rs         # Public API types
├── context.rs       # Request context and configuration
├── session.rs       # Session management
├── indexer.rs       # Document indexing operations
├── retriever.rs     # Query and retrieval operations
├── workspace.rs     # Workspace operations (CRUD)
└── events.rs        # Event system and callbacks
```

### Architecture Diagram

```
┌─────────────────────────────────────────────────────────────────┐
│                           Client API                             │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  ┌──────────────┐    ┌──────────────┐    ┌──────────────┐       │
│  │ EngineBuilder │───▶│    Engine    │◀───│   Session    │       │
│  └──────────────┘    └──────┬───────┘    └──────────────┘       │
│                             │                                    │
│              ┌──────────────┼──────────────┐                    │
│              ▼              ▼              ▼                    │
│     ┌─────────────┐ ┌─────────────┐ ┌─────────────┐            │
│     │   Indexer   │ │  Retriever  │ │  Workspace  │            │
│     │   Client    │ │   Client    │ │   Client    │            │
│     └──────┬──────┘ └──────┬──────┘ └──────┬──────┘            │
│            │               │               │                    │
│            └───────────────┴───────────────┘                    │
│                            │                                    │
│                            ▼                                    │
│                   ┌────────────────┐                           │
│                   │    Context     │                           │
│                   │  (Request State)│                           │
│                   └────────────────┘                           │
│                                                                   │
│                   ┌────────────────┐                           │
│                   │    Events      │                           │
│                   │  (Callbacks)   │                           │
│                   └────────────────┘                           │
└─────────────────────────────────────────────────────────────────┘
```

---

## Component Design

### 1. Context (`context.rs`)

Request-scoped configuration and state management.

```rust
/// Request context for client operations.
pub struct ClientContext {
    /// Unique request ID for tracing.
    pub request_id: Uuid,

    /// Request-specific configuration overrides.
    pub config: RequestContextConfig,

    /// Event emitter for this request.
    pub events: EventEmitter,

    /// Request metadata.
    pub metadata: HashMap<String, String>,

    /// Request deadline (for timeout).
    pub deadline: Option<Instant>,
}

/// Request-specific configuration overrides.
pub struct RequestContextConfig {
    /// Override top_k for retrieval.
    pub top_k: Option<usize>,

    /// Override token budget.
    pub token_budget: Option<usize>,

    /// Override content format.
    pub content_format: Option<ContentFormat>,

    /// Enable/disable features.
    pub features: FeatureFlags,
}

/// Feature flags for request.
pub struct FeatureFlags {
    pub include_summaries: bool,
    pub include_content: bool,
    pub enable_cache: bool,
    pub enable_sufficiency_check: bool,
}
```

### 2. Session (`session.rs`)

Multi-document session management.

```rust
/// Session for managing multiple document operations.
pub struct Session {
    /// Session ID.
    pub id: Uuid,

    /// Session configuration.
    config: SessionConfig,

    /// Active document contexts.
    documents: HashMap<String, DocumentContext>,

    /// Shared engine reference.
    engine: Engine,

    /// Session statistics.
    stats: SessionStats,

    /// Created at timestamp.
    created_at: DateTime<Utc>,
}

/// Document context within a session.
pub struct DocumentContext {
    /// Document ID.
    pub doc_id: String,

    /// Preloaded tree (cached).
    tree: Option<Arc<DocumentTree>>,

    /// Document metadata.
    meta: DocumentMeta,

    /// Access statistics.
    access_count: usize,
    last_accessed: DateTime<Utc>,
}

/// Session configuration.
pub struct SessionConfig {
    /// Maximum documents to keep in memory.
    pub max_cached_documents: usize,

    /// Preload strategy.
    pub preload_strategy: PreloadStrategy,

    /// Cache eviction policy.
    pub eviction_policy: EvictionPolicy,
}

impl Session {
    /// Create a new session.
    pub fn new(engine: Engine) -> Self;

    /// Index a document into this session.
    pub async fn index(&self, path: impl AsRef<Path>) -> Result<String>;

    /// Query a document within this session.
    pub async fn query(&self, doc_id: &str, question: &str) -> Result<QueryResult>;

    /// Query across all documents in session.
    pub async fn query_all(&self, question: &str) -> Result<Vec<QueryResult>>;

    /// Get document tree (cached).
    pub fn get_tree(&self, doc_id: &str) -> Result<Arc<DocumentTree>>;

    /// Preload documents for faster access.
    pub async fn preload(&self, doc_ids: &[&str]) -> Result<()>;

    /// Clear session cache.
    pub fn clear_cache(&self);

    /// Get session statistics.
    pub fn stats(&self) -> &SessionStats;
}
```

### 3. Indexer Client (`indexer.rs`)

Document indexing operations.

```rust
/// Document indexing client.
pub struct IndexerClient {
    /// Pipeline executor.
    executor: Arc<Mutex<PipelineExecutor>>,

    /// Configuration.
    config: IndexerConfig,
}

/// Indexing configuration.
pub struct IndexerConfig {
    /// Default index mode.
    pub default_mode: IndexMode,

    /// Summary generation strategy.
    pub summary_strategy: SummaryStrategy,

    /// Whether to generate node IDs.
    pub generate_ids: bool,

    /// Whether to generate descriptions.
    pub generate_descriptions: bool,
}

impl IndexerClient {
    /// Create a new indexer client.
    pub fn new(executor: PipelineExecutor) -> Self;

    /// Index a document from file.
    pub async fn index_file(
        &self,
        path: impl AsRef<Path>,
        options: IndexOptions,
        events: &EventEmitter,
    ) -> Result<IndexedDocument>;

    /// Index from raw content.
    pub async fn index_content(
        &self,
        content: &str,
        format: DocumentFormat,
        options: IndexOptions,
    ) -> Result<IndexedDocument>;

    /// Detect document format.
    pub fn detect_format(&self, path: &Path, options: &IndexOptions) -> Result<DocumentFormat>;

    /// Validate document before indexing.
    pub fn validate(&self, path: &Path) -> Result<ValidationResult>;
}

/// Indexing events.
pub enum IndexEvent {
    /// Started indexing.
    Started { path: String },

    /// Format detected.
    FormatDetected { format: DocumentFormat },

    /// Parsing progress.
    ParsingProgress { percent: u8 },

    /// Tree building complete.
    TreeBuilt { node_count: usize },

    /// Summary generation progress.
    SummaryProgress { completed: usize, total: usize },

    /// Indexing complete.
    Complete { doc_id: String },

    /// Error occurred.
    Error { message: String },
}
```

### 4. Retriever Client (`retriever.rs`)

Query and retrieval operations.

```rust
/// Document retrieval client.
pub struct RetrieverClient {
    /// Pipeline retriever.
    retriever: Arc<PipelineRetriever>,

    /// Configuration.
    config: RetrieverConfig,
}

/// Retrieval configuration.
pub struct RetrieverConfig {
    /// Default top_k.
    pub default_top_k: usize,

    /// Default token budget.
    pub default_token_budget: usize,

    /// Content aggregator config.
    pub content_config: ContentAggregatorConfig,

    /// Enable caching.
    pub enable_cache: bool,
}

impl RetrieverClient {
    /// Create a new retriever client.
    pub fn new(retriever: PipelineRetriever) -> Self;

    /// Query a document tree.
    pub async fn query(
        &self,
        tree: &DocumentTree,
        question: &str,
        options: RetrieveOptions,
        ctx: &ClientContext,
    ) -> Result<QueryResult>;

    /// Query with streaming results.
    pub async fn query_stream(
        &self,
        tree: &DocumentTree,
        question: &str,
        options: RetrieveOptions,
    ) -> impl Stream<Item = QueryEvent>;

    /// Get similar nodes.
    pub fn find_similar(
        &self,
        tree: &DocumentTree,
        node_id: NodeId,
        top_k: usize,
    ) -> Result<Vec<RetrievalResult>>;

    /// Get node context (ancestors + siblings).
    pub fn get_node_context(
        &self,
        tree: &DocumentTree,
        node_id: NodeId,
        depth: usize,
    ) -> Result<NodeContext>;
}

/// Query events for streaming.
pub enum QueryEvent {
    /// Search started.
    SearchStarted { query: String },

    /// Node visited during search.
    NodeVisited { node_id: String, title: String, score: f32 },

    /// Candidate found.
    CandidateFound { node_id: String, score: f32 },

    /// Sufficiency check result.
    SufficiencyCheck { level: SufficiencyLevel, tokens: usize },

    /// Result ready.
    ResultReady { result: RetrievalResult },

    /// Query complete.
    Complete { total_results: usize, confidence: f32 },
}
```

### 5. Workspace Client (`workspace.rs`)

Document persistence operations.

```rust
/// Workspace management client.
pub struct WorkspaceClient {
    /// Workspace storage.
    workspace: Arc<RwLock<Workspace>>,

    /// Configuration.
    config: WorkspaceConfig,
}

/// Workspace configuration.
pub struct WorkspaceConfig {
    /// Auto-save interval (seconds).
    pub auto_save_interval: Option<u64>,

    /// Maximum cache size.
    pub max_cache_size: usize,
}

impl WorkspaceClient {
    /// Create a new workspace client.
    pub fn new(workspace: Workspace) -> Self;

    /// Save a document.
    pub fn save(&self, doc: &PersistedDocument) -> Result<()>;

    /// Load a document.
    pub fn load(&self, doc_id: &str) -> Result<Option<PersistedDocument>>;

    /// Remove a document.
    pub fn remove(&self, doc_id: &str) -> Result<bool>;

    /// Check if document exists.
    pub fn exists(&self, doc_id: &str) -> Result<bool>;

    /// List all documents.
    pub fn list(&self) -> Result<Vec<DocumentInfo>>;

    /// Get document metadata.
    pub fn get_meta(&self, doc_id: &str) -> Result<Option<DocumentMeta>>;

    /// Batch operations.
    pub fn batch_remove(&self, doc_ids: &[&str]) -> Result<usize>;

    /// Clear workspace.
    pub fn clear(&self) -> Result<usize>;

    /// Get workspace statistics.
    pub fn stats(&self) -> WorkspaceStats;
}

/// Workspace statistics.
pub struct WorkspaceStats {
    pub document_count: usize,
    pub total_size_bytes: u64,
    pub cache_hit_rate: f32,
    pub oldest_document: Option<DateTime<Utc>>,
    pub newest_document: Option<DateTime<Utc>>,
}
```

### 6. Events (`events.rs`)

Event system for callbacks and progress reporting.

```rust
/// Event emitter for client operations.
pub struct EventEmitter {
    /// Event handlers.
    handlers: Vec<Box<dyn EventHandler>>,

    /// Async handlers (for non-blocking events).
    async_handlers: Vec<Arc<dyn AsyncEventHandler>>,
}

/// Event handler trait.
pub trait EventHandler: Send + Sync {
    fn handle(&self, event: &Event);
}

/// Async event handler trait.
#[async_trait]
pub trait AsyncEventHandler: Send + Sync {
    async fn handle(&self, event: &Event);
}

/// Event types.
#[derive(Debug, Clone)]
pub enum Event {
    /// Indexing events.
    Index(IndexEvent),

    /// Query events.
    Query(QueryEvent),

    /// Workspace events.
    Workspace(WorkspaceEvent),

    /// Session events.
    Session(SessionEvent),
}

/// Workspace events.
pub enum WorkspaceEvent {
    DocumentSaved { doc_id: String },
    DocumentLoaded { doc_id: String, cache_hit: bool },
    DocumentRemoved { doc_id: String },
    WorkspaceCleared { count: usize },
}

/// Session events.
pub enum SessionEvent {
    SessionCreated { session_id: Uuid },
    DocumentAdded { doc_id: String },
    DocumentEvicted { doc_id: String, reason: EvictionReason },
    SessionClosed { session_id: Uuid },
}

impl EventEmitter {
    /// Create a new event emitter.
    pub fn new() -> Self;

    /// Add a sync handler.
    pub fn on<H: EventHandler + 'static>(mut self, handler: H) -> Self;

    /// Add an async handler.
    pub fn on_async<H: AsyncEventHandler + 'static>(mut self, handler: Arc<H>) -> Self;

    /// Emit an event.
    pub fn emit(&self, event: Event);

    /// Emit an event asynchronously.
    pub async fn emit_async(&self, event: Event);
}

/// Convenience handler builders.
impl EventEmitter {
    /// Create handler from closure.
    pub fn on_index<F: Fn(&IndexEvent) + Send + Sync + 'static>(self, f: F) -> Self;

    /// Create handler from closure.
    pub fn on_query<F: Fn(&QueryEvent) + Send + Sync + 'static>(self, f: F) -> Self;

    /// Create progress callback.
    pub fn on_progress<F: Fn(Progress) + Send + Sync + 'static>(self, f: F) -> Self;
}

/// Progress information.
pub struct Progress {
    pub operation: Operation,
    pub current: usize,
    pub total: usize,
    pub message: String,
}

pub enum Operation {
    Indexing,
    Querying,
    Loading,
    Saving,
}
```

### 7. Simplified Engine (`engine.rs`)

The main orchestrator, now much simpler.

```rust
/// The main Engine client - orchestrates sub-clients.
pub struct Engine {
    /// Configuration.
    config: Arc<Config>,

    /// Indexer client.
    indexer: IndexerClient,

    /// Retriever client.
    retriever: RetrieverClient,

    /// Workspace client (optional).
    workspace: Option<WorkspaceClient>,

    /// Event emitter.
    events: EventEmitter,
}

impl Engine {
    /// Create a builder for custom configuration.
    pub fn builder() -> EngineBuilder;

    // ============================================================
    // Convenience Methods (delegate to sub-clients)
    // ============================================================

    /// Index a document.
    pub async fn index(&self, path: impl AsRef<Path>) -> Result<String> {
        self.index_with_options(path, IndexOptions::default()).await
    }

    /// Index with options.
    pub async fn index_with_options(
        &self,
        path: impl AsRef<Path>,
        options: IndexOptions,
    ) -> Result<String>;

    /// Query a document.
    pub async fn query(&self, doc_id: &str, question: &str) -> Result<QueryResult>;

    /// Create a session for multi-document operations.
    pub fn session(&self) -> Session;

    /// Get the indexer client.
    pub fn indexer(&self) -> &IndexerClient;

    /// Get the retriever client.
    pub fn retriever(&self) -> &RetrieverClient;

    /// Get the workspace client.
    pub fn workspace(&self) -> Option<&WorkspaceClient>;

    /// Get configuration.
    pub fn config(&self) -> &Config;

    // ============================================================
    // Document Operations (delegate to workspace)
    // ============================================================

    /// List documents.
    pub fn list_documents(&self) -> Vec<DocumentInfo>;

    /// Get document structure.
    pub fn get_structure(&self, doc_id: &str) -> Result<DocumentTree>;

    /// Get page content.
    pub fn get_page_content(&self, doc_id: &str, pages: &str) -> Result<String>;

    /// Remove document.
    pub fn remove(&self, doc_id: &str) -> Result<bool>;

    /// Check existence.
    pub fn exists(&self, doc_id: &str) -> Result<bool>;
}
```

---

## API Examples

### Basic Usage (Same as Before)

```rust
let client = EngineBuilder::new()
    .with_workspace("./workspace")
    .build()?;

// Index
let doc_id = client.index("./document.md").await?;

// Query
let result = client.query(&doc_id, "What is this?").await?;
```

### With Events

```rust
let client = EngineBuilder::new()
    .with_workspace("./workspace")
    .with_events(
        EventEmitter::new()
            .on_index(|e| match e {
                IndexEvent::Complete { doc_id } => println!("Indexed: {}", doc_id),
                _ => {}
            })
            .on_query(|e| match e {
                QueryEvent::NodeVisited { title, score, .. } => {
                    println!("Visited: {} (score: {:.2})", title, score);
                }
                _ => {}
            })
    )
    .build()?;
```

### Session-Based Multi-Document

```rust
let client = EngineBuilder::new()
    .with_workspace("./workspace")
    .build()?;

// Create session
let session = client.session();

// Index multiple documents
let doc1 = session.index("./doc1.md").await?;
let doc2 = session.index("./doc2.md").await?;
let doc3 = session.index("./doc3.md").await?;

// Query across all documents
let results = session.query_all("What is the architecture?").await?;

// Query single document (cached tree)
let result = session.query(&doc1, "Summary?").await?;

// Session stats
println!("Cache hit rate: {:.2}%", session.stats().cache_hit_rate * 100.0);
```

### Streaming Query

```rust
let client = EngineBuilder::new()
    .with_workspace("./workspace")
    .build()?;

// Stream query results
let mut stream = client.retriever()
    .query_stream(&tree, "What is X?", RetrieveOptions::default());

while let Some(event) = stream.next().await {
    match event {
        QueryEvent::NodeVisited { title, score, .. } => {
            println!("Exploring: {}", title);
        }
        QueryEvent::ResultReady { result } => {
            println!("Found: {}", result.title);
        }
        QueryEvent::Complete { total_results, confidence } => {
            println!("Done: {} results, confidence: {:.2}", total_results, confidence);
        }
        _ => {}
    }
}
```

### Request Context

```rust
let ctx = ClientContext::new()
    .with_top_k(10)
    .with_token_budget(8000)
    .with_deadline(Duration::from_secs(30));

let result = client.retriever()
    .query(&tree, "complex question", options, &ctx)
    .await?;
```

---

## Migration Path

### Phase 1: Add New Modules (Non-Breaking)
1. Create `context.rs`, `events.rs`
2. Create `indexer.rs`, `retriever.rs`, `workspace.rs` as wrappers
3. Update `engine.rs` to use sub-clients internally
4. All existing API remains unchanged

### Phase 2: Add Session Support (Non-Breaking)
1. Add `session.rs`
2. Add `Engine::session()` method
3. Add multi-document query support

### Phase 3: Enhance Events (Non-Breaking)
1. Add streaming query support
2. Add progress callbacks
3. Add async event handlers

### Phase 4: Deprecate Old API (Breaking, Future)
1. Mark direct workspace access as deprecated
2. Encourage use of sub-clients
3. Eventually remove deprecated methods

---

## File Structure After Refactoring

```
src/client/
├── mod.rs           # ~50 lines - exports and docs
├── engine.rs        # ~150 lines - orchestration only
├── builder.rs       # ~200 lines - enhanced builder
├── types.rs         # ~250 lines - public types
├── context.rs       # ~150 lines - request context
├── session.rs       # ~200 lines - session management
├── indexer.rs       # ~200 lines - indexing ops
├── retriever.rs     # ~200 lines - retrieval ops
├── workspace.rs     # ~150 lines - workspace ops
└── events.rs        # ~200 lines - event system
```

Total: ~1750 lines (vs current ~1000 lines, but much better organized)

---

## Benefits

1. **Single Responsibility**: Each module has one clear purpose
2. **Testability**: Sub-clients can be tested independently
3. **Extensibility**: Easy to add new features without touching Engine
4. **Performance**: Session caching reduces redundant loads
5. **Observability**: Events provide visibility into operations
6. **API Clarity**: Clear separation between indexing, retrieval, and storage
7. **Streaming**: Support for progressive results
8. **Context Management**: Request-scoped configuration
