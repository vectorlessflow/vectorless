# 架构评估与路线图

> 评估日期: 2026-04-03
> 评估版本: v0.1.7

## 当前状态

| 指标 | 状态 |
|------|------|
| **测试** | 129 passed, 0 failed |
| **代码量** | 17,695 行 Rust (112 文件) |
| **模块** | client, domain, index, retrieval, llm, parser, storage, throttle |
| **编译** | 成功 (仅 warnings) |

## 架构亮点

### 1. 双 Pipeline 设计一致

Index 和 Retrieval 都采用相同的 orchestrator 模式:
- 依赖解析 (topological sort)
- ExecutionGroup 支持并行
- FailurePolicy (Fail/Skip/Retry)
- StageOutcome 流程控制

```
┌─────────────────────────────────────────────────────────────┐
│                     Orchestrator 模式                        │
├─────────────────────────────────────────────────────────────┤
│  Index Pipeline          │  Retrieval Pipeline              │
│  ─────────────           │  ─────────────────               │
│  Parse → Build →         │  Analyze → Plan →                │
│  Enhance → Enrich →      │  Search → Judge                  │
│  Optimize                │  (支持回溯)                       │
└─────────────────────────────────────────────────────────────┘
```

### 2. 清晰的分层架构

```
client (Engine) → index/retrieval → domain ← parser/llm/config
```

- **client**: 高层 API，封装内部复杂性
- **domain**: 核心领域类型，无外部依赖
- **index/retrieval**: 业务逻辑，操作 domain
- **parser/llm/config**: 基础设施，提供能力

### 3. 良好的模块化

每个模块职责单一:
- `parser/` - 文档解析 (Markdown, PDF, DOCX)
- `llm/` - LLM 客户端 (retry, fallback, pool)
- `storage/` - 持久化 (Workspace, LRU cache)
- `throttle/` - 限流控制

---

## 待改进项

### 代码质量 (Clippy Warnings)

| 类型 | 数量 | 示例 |
|------|------|------|
| unused variable | 8 | `_context`, `_query`, `_strategy` |
| dead_code | 5 | `find_stage_index`, `term_frequency` |
| must_use | 12 | builder 方法缺少 `#[must_use]` |
| style | 3 | redundant else, unnecessary hashes |

### 功能缺失

| 模块 | 缺失 | 影响 |
|------|------|------|
| `parser/registry.rs` | HTML parser | HTML 格式不支持 |
| `parser/toc/processor.rs` | 无 ToC 文档的结构提取 | 依赖 LLM |
| `retrieval/strategy/llm.rs` | 批量 prompt 优化 | 性能 |

### 架构限制

| 限制 | 说明 |
|------|------|
| **并行执行未实现** | ExecutionGroup 已设计但 `execute()` 仍顺序执行 |
| **Strategy 无切换** | Plan 选择策略后中途不可切换 |
| **增量索引骨架** | `ChangeDetector` 存在但未集成到 pipeline |

---

## 下一阶段优化方案

### Phase 1: 代码清理 (优先级: 高)

**目标**: 消除所有 clippy warnings

| 任务 | 文件 | 工作量 |
|------|------|--------|
| 添加 `#[must_use]` | builder 类型 | ~12 处 |
| 修复 unused variables | 各模块 | ~8 处 |
| 移除 dead code | `search/mod.rs`, `strategy/keyword.rs` | ~5 处 |
| 修复 style issues | 散落各处 | ~3 处 |

**验收标准**: `cargo clippy` 无 warnings

---

### Phase 2: 功能补全 (优先级: 中)

#### 2.1 HTML Parser

```rust
// src/parser/html/mod.rs (新建)
pub struct HtmlParser {
    config: HtmlConfig,
}

impl DocumentParser for HtmlParser {
    fn parse(&self, content: &str) -> ParseResult {
        // 使用 html5ever 或 scraper crate
    }
}
```

#### 2.2 Strategy 热切换

当前: Plan 阶段选择策略后固定
目标: Search 阶段根据效果动态切换

```rust
// 在 SearchStage 中
if current_strategy.is_struggling() {
    ctx.switch_strategy(Strategy::more_capable());
}
```

#### 2.3 增量索引集成

```rust
// 在 PipelineExecutor 中
pub fn execute_incremental(
    &mut self,
    input: IndexInput,
    changes: ChangeSet,
) -> Result<IndexResult> {
    // 只处理变更部分
}
```

---

### Phase 3: 性能优化 (优先级: 中)

#### 3.1 并行执行实现

**当前状态**: `ExecutionGroup` 已设计，但 `execute()` 仍顺序执行

```rust
// 当前 (顺序)
for &stage_idx in &group.stage_indices {
    entry.stage.execute(&mut ctx).await?;
}

// 目标 (并行)
futures::future::try_join_all(
    group.stage_indices.iter()
        .map(|&idx| self.stages[idx].execute(&ctx))
).await?;
```

**挑战**:
- `PipelineContext` 需要 `Send + Sync`
- 需要细粒度锁或消息传递

#### 3.2 Path Cache 命中率

```rust
// 添加热点查询缓存
pub struct PathCache {
    entries: LruCache<QueryHash, CachedPath>,
    hot_queries: Arc<RwLock<HashSet<QueryHash>>>, // 新增
}
```

#### 3.3 批量 LLM 调用

```rust
// 当前: 逐个评估
for node_id in node_ids {
    self.evaluate_node(tree, node_id, context).await;
}

// 目标: 批量评估
self.evaluate_nodes_batch(tree, node_ids, context).await;
```

---

### Phase 4: 测试增强 (优先级: 低)

| 测试类型 | 当前 | 目标 |
|----------|------|------|
| 单元测试 | 129 | +50 |
| 集成测试 | 0 (仅 examples) | +10 |
| Property 测试 | 0 | +5 |
| 覆盖率报告 | 无 | cargo-tarpaulin |

---

## 执行顺序

```
Phase 1 (代码清理)
    ↓
Phase 3.1 (并行执行)
    ↓
Phase 2 (功能补全)
    ↓
Phase 4 (测试增强)
```

**建议首先执行 Phase 1 代码清理**，消除所有 clippy warnings，使代码库更干净。

---

## 文件变更预览

### Phase 1 涉及文件

```
src/
├── client/builder.rs          # 添加 #[must_use]
├── config/types.rs            # 添加 #[must_use]
├── domain/tree.rs             # 移除 dead code
├── index/
│   ├── pipeline/orchestrator.rs  # 移除 find_stage_index
│   └── stages/*.rs               # 修复 unused
├── retrieval/
│   ├── search/mod.rs          # 移除 dead code
│   ├── strategy/keyword.rs    # 移除 term_frequency
│   └── stages/*.rs            # 修复 unused
└── llm/client.rs              # 修复 unused max_tokens
```

---

## 参考资料

- [Architecture v2](./architecture-v2.svg)
- [Pipeline Design](./v2.md)
- [RFCs](../rfcs/)
