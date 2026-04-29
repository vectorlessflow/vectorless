# HISTORY

## 0.1.15 (2026-04-29)

- **Python bindings**: enabled PyO3 abi3 for cross-version compatibility
- Single wheel now supports Python 3.11, 3.12, and 3.13+
- Simplified exception type using `create_exception!` macro

## 0.1.14 (2026-04-29)

- Homepage redesign with product showcase and improved UX
- Chain command and intent-based navigation hints for agent navigation
- Parser registry and custom format support
- Standardized import order and code formatting across modules
- Updated package metadata and documentation links
- Single-document reasoning challenge example with README
- Removed redundant documentation links and unused test

## 0.1.13 (2026-04-26)

- **Compiler terminology**: renamed `index` to `compile`, `reindexing` to `recompilation`
- Removed unused incremental updater and summary strategies
- Moved keyword extraction from scoring to utils module
- Added tracing and logging for better debugging experience
- Workspace restructuring: renamed `vectorless-core` to `crates`
- Removed commented-out strategy layer crates from Cargo.toml

## 0.1.12 (2026-04-24)

- **Compile pipeline**: renamed index pipeline to compile pipeline with passes-based architecture
- **Compiler refactor**: renamed stages to passes, removed deprecated `StageResult` alias and `CustomStageBuilder`
- New backend compilation passes: query routing, reasoning chains, overlap detection, and scoring
- Agent acceleration data added to compiled documents
- LLM-powered cross-document insight extraction in ask module
- Enhanced JSON parsing with proper error handling
- Upgraded minimum Python version to 3.11
- Removed unused modules: agent, memory backend, validation, ReferenceResolver, SufficiencyLevel
- Restructured configuration modules and removed legacy retrieval config
- Simplified storage layer by removing memory backend
- Documentation updates for architecture and compilation pipeline

## 0.1.11 (2026-04-21)

- Project description updated to "reasoning-based document engine"
- Core principles documentation (Reason don't vector, Model fails we fail, No thought no answer)
- Updated homepage with three core principles and key features

## 0.1.10 (2026-04-21)

- Description generation enabled by default
- `timeout_secs` option for Python indexing
- Agent-based navigation documentation

## 0.1.9 (2026-04-20)

- **Agent-based retrieval architecture**: replaced pilot/search with Orchestrator + Workers
- Navigation commands: `ls`, `cd`, `cat`, `grep`, `find`, `head`, `pwd`, `wc`
- Orchestrator supervisor loop with dynamic re-planning
- Query understanding pipeline with `QueryPlan`
- Evidence evaluation and replanning modules
- `NavigationIndex` with `DocCard` and `SectionCard`
- LLM-based confidence scoring (replaced BM25)
- Unified rerank pipeline (replaced synthesis/fusion)
- `DocCard` catalog in workspace storage
- Shared concurrency control for LLM clients
- Memoization for LLM operations in retrieval pipeline
- LLM request timeout configuration

## 0.1.8 (2026-04-16)

- GitHub Actions workflow for automated releases
- Endpoint parameter support for API configuration
- Custom config option in `EngineBuilder`
- Enhanced error messages with detailed failure info
- Endpoint validation in engine builder

## 0.1.7 (2026-04-15)

- Runtime metrics reports (LLM, Pilot, Retrieval)
- Recursive option for `from_dir` method
- Directory indexing support via `IndexContext`
- Centralized `LlmPool` configuration system
- Shared LLM client injected into pipeline context
- Pipeline checkpoint for resumable indexing
- `source_path` field and updated `QueryContext` API

## 0.1.6 (2026-04-15)

- `IndexMetrics` binding with detailed indexing statistics
- `StrategyPreference` for controlling retrieval strategies
- Pure Pilot search algorithm, beam search with backtracking
- Per-step reasoning support in search algorithms
- Binary pruning and pre-filtering for wide nodes
- LLM-based query complexity detection
- Cross-document strategy with graph-based boosting
- Synonym expansion for improved query recall
- Default summary strategy changed to Full

## 0.1.4 (2026-04-13)

- PDF parser: switch to `pdf-extract` for reliable text extraction
- Concurrent LLM verification for TOC entries
- PDF indexing example

## 0.1.3 (2026-04-13)

- Internal module naming cleanup (`_` prefix for private functions)

## 0.1.2 (2026-04-13)

- Search-from functionality and ToC-based navigation
- Reasoning chain (replacing navigation trace)
- Adaptive budget controller for pipeline token management
- Structural path constraints and hints extraction
- Reasoning index for fast retrieval path resolution
- Document graph system for cross-document relationships
- Streaming retrieval with `RetrieveEvent` support
- Multi-document query support
- Incremental indexing with content and logic fingerprinting
- Parallel processing for multiple document sources
- Pipeline checkpoint and content merging/splitting support

## 0.1.1 (2026-04-08)

- Workspace-managed dependencies and configuration
- LLM pilot functionality and summary generation
- Query decomposition support
- LLM-first search with TOC-based location
- Restructured Python examples

## 0.1.0 (2026-04-07)

Initial Python SDK release.

- PyO3 bindings for the Rust engine core
- Basic `Engine` class with `index()` and `query()` methods
- `pyproject.toml` with maturin build backend
- Ruff formatting configuration
