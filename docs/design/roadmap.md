# 架构评估与路线图

> 评估日期: 2026-04-04
> 评估版本: v0.2.0

## 当前状态

| 指标 | 状态 |
|------|------|
| **测试** | 197 passed, 0 failed |
| **代码量** | 26,000+ 行 Rust |
| **模块** | client, domain, index, retrieval, pilot, llm, parser, storage, throttle |
| **编译** | 成功 |

## 架构亮点

### 1. 双 Pipeline 设计完整

Index 和 Retrieval 都采用相同的 orchestrator 模式:
- 依赖解析 (topological sort)
- ExecutionGroup 支持并行
- FailurePolicy (Fail/Skip/Retry)
- StageOutcome 流程控制
- **Backtracking 支持** (Retrieval)

```
┌─────────────────────────────────────────────────────────────┐
│                     Orchestrator 模式                        │
├─────────────────────────────────────────────────────────────┤
│  Index Pipeline           │  Retrieval Pipeline              │
│  ─────────────            │  ─────────────────               │
│  Parse → Build →          │  Analyze → Plan →                │
│  Enhance → Enrich →       │  Search → Judge                  │
│  Optimize                 │  (支持回溯 + Pilot)               │
└─────────────────────────────────────────────────────────────┘
```

### 2. Pilot 模块完整实现

**Pilot 是 Retrieval Pipeline 的"大脑"**，负责语义理解和导航决策：

```
┌─────────────────────────────────────────────────────────────┐
│                     Pilot 架构                               │
├─────────────────────────────────────────────────────────────┤
│  干预点: START → FORK → BACKTRACK → EVALUATE                │
│  组件: BudgetController, ContextBuilder, FallbackManager    │
│  特性: 分数合并, 4级降级策略, 指标收集                        │
└─────────────────────────────────────────────────────────────┘
```

**核心设计理念**:
- Algorithm 处理 "how to search" — 高效、确定性
- Pilot 处理 "where to go" — 语义理解、方向指引
- 只在关键决策点干预，不是每一步

### 3. 清晰的分层架构

```
client (Engine) → index/retrieval → domain ← parser/llm/config
                              ↑
                           pilot (大脑)
```

- **client**: 高层 API，封装内部复杂性
- **domain**: 核心领域类型，无外部依赖
- **index/retrieval**: 业务逻辑，操作 domain
- **pilot**: LLM 导航智能，干预检索流程
- **parser/llm/config**: 基础设施，提供能力

---

## 已完成功能

| 功能 | 状态 | 说明 |
|------|------|------|
| Index Pipeline | ✅ | Parse, Build, Enhance, Enrich, Optimize |
| Retrieval Pipeline | ✅ | Analyze, Plan, Search, Judge |
| Backtracking | ✅ | NeedMoreData, 显式 Backtrack |
| Pilot Trait | ✅ | should_intervene, decide, guide_* |
| BudgetController | ✅ | Token/Call 限制，预算分配 |
| FallbackManager | ✅ | 4级降级策略 |
| MetricsCollector | ✅ | 延迟、Token、成功率追踪 |
| Score Merging | ✅ | α×algo + β×llm |
| Markdown Parser | ✅ | 完整支持 |
| PDF Parser | ✅ | 基于 pdf-extract |
| DOCX Parser | ✅ | 基于 docx-rs |

---

## 待改进项

### 功能缺失

| 模块 | 缺失 | 优先级 |
|------|------|--------|
| `parser/` | HTML parser | 中 |
| `parser/` | Plain text parser | 低 |
| `retrieval/strategy/` | 批量 prompt 优化 | 中 |

### 架构限制

| 限制 | 说明 | 优先级 |
|------|------|--------|
| **并行执行未实现** | ExecutionGroup 已设计但 `execute()` 仍顺序执行 | 高 |
| **Strategy 无切换** | Plan 选择策略后中途不可切换 | 低 |
| **增量索引骨架** | `ChangeDetector` 存在但未集成到 pipeline | 低 |

---

## 下一阶段路线图

### Phase 1: 性能基准 (当前)

**目标**: 建立性能基准，为优化提供依据

| 任务 | 文件 | 状态 |
|------|------|------|
| Index 性能基准 | `benches/index_bench.rs` | 📝 待实现 |
| Retrieval 性能基准 | `benches/retrieval_bench.rs` | 📝 待实现 |
| Pilot 性能基准 | `benches/pilot_bench.rs` | 📝 待实现 |
| Token 消耗基准 | `benches/token_bench.rs` | 📝 待实现 |

---

### Phase 2: 性能优化

**目标**: 基于基准测试结果优化关键路径

#### 2.1 并行执行实现

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

#### 2.2 Pilot 调用优化

```rust
// 当前: 逐个评估
for node_id in node_ids {
    pilot.evaluate_node(tree, node_id).await;
}

// 目标: 批量评估
pilot.evaluate_nodes_batch(tree, node_ids).await;
```

#### 2.3 缓存优化

- Path Cache 命中率优化
- 热点查询缓存
- LLM 响应缓存 (相同上下文)

---

### Phase 3: 功能补全

#### 3.1 HTML Parser

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

#### 3.2 更多 LLM Provider

- Anthropic Claude API
- Local LLM (Ollama, llama.cpp)
- Azure OpenAI

#### 3.3 流式输出

```rust
// 支持流式检索结果
pub async fn query_stream(
    &self,
    doc_id: &DocumentId,
    query: &str,
) -> impl Stream<Item = RetrieveEvent> {
    // 边检索边返回
}
```

---

### Phase 4: 示例完善

| 示例 | 说明 | 状态 |
|------|------|------|
| `basic.rs` | 基础用法 | ✅ 已有 |
| `index.rs` | 索引文档 | ✅ 已有 |
| `retrieve.rs` | 检索文档 | ✅ 已有 |
| `markdownflow.rs` | Markdown 流程 | ✅ 已有 |
| `custom_pilot.rs` | 自定义 Pilot | 📝 待实现 |
| `batch_processing.rs` | 批量处理 | 📝 待实现 |
| `streaming.rs` | 流式输出 | 📝 待实现 |
| `multi_format.rs` | 多格式文档 | 📝 待实现 |
| `cli_tool.rs` | CLI 工具示例 | 📝 待实现 |

---

### Phase 5: 测试增强

| 测试类型 | 当前 | 目标 |
|----------|------|------|
| 单元测试 | 197 | +30 |
| 集成测试 | 1 | +10 |
| 基准测试 | 0 | +4 |
| 覆盖率报告 | 无 | cargo-tarpaulin |

---

## 执行顺序

```
Phase 1 (性能基准) ← 当前
    ↓
Phase 2 (性能优化)
    ↓
Phase 3 (功能补全)
    ↓
Phase 4 (示例完善)
    ↓
Phase 5 (测试增强)
```

**建议首先建立性能基准**，这样才能：
1. 发现真正的瓶颈
2. 衡量优化效果
3. 防止性能回归

---

## 参考资料

- [Architecture v2](./architecture-v2.svg)
- [Pilot Architecture](./pilot-architecture.svg)
- [Pipeline Design](./v2.md)
- [Pilot Design](./pilot.md)
- [RFCs](../rfcs/)
