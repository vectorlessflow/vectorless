# 建议新增/强化的功能

> 基于 Vectorless 定位：ultra-performant reasoning-native document intelligence engine，no vectors。

---

## Tier 1: 核心差异化 — "推理原生" 的灵魂

### 1. 结构化推理链 (Reasoning Chain)

检索过程输出可解释的推理路径：

```
Query → 路径选择 → 节点评估 → 扩展/回溯 → 结论
```

- 不只是返回结果，而是返回"为什么找到这些内容"的完整链路
- 支持用户审查和修正推理路径（human-in-the-loop）
- 每一步记录：决策依据、LLM prompt/response 摘要、候选节点列表

**价值**: 这是和 vector RAG 的根本区别 — 不是黑盒相似度，而是可审计的结构推理。

### 2. 查询规划器 (Query Planner)

对复杂查询自动分解为子查询树：

- 识别查询中的结构线索（"第三章的方法"、"和上一节对比"）
- 支持多文档交叉引用推理（"在 A 文档提到的概念，B 文档如何定义"）
- 将自然语言查询映射为树上的检索计划

**价值**: 从"搜索单一问题"升级为"规划研究路径"。

### 3. 多跳推理 (Multi-hop Reasoning)

自动追踪文档内和跨文档的引用链：

- 文档内交叉引用（"详见 Section 4.2"、"如表 3 所示"）
- 跨文档引用链追踪
- 检索结果不足时自动发起补充查询

**价值**: 人类读文档时会来回翻页跳转，Vectorless 应该模拟这种行为。

---

## Tier 2: 性能 — "Ultra-performant" 的支撑

### 4. 预计算推理索引 (Reasoning Index)

在索引阶段预计算，降低运行时 LLM 调用成本：

- 预计算高频查询路径（类似查询物化视图）
- 节点间语义桥接（预计算哪些节点之间有强关联）
- 基于 TOC 的查询路由表（哪些章节处理哪些主题）

**价值**: 将运行时推理成本转移到索引时，查询时零 LLM 调用或极少调用。

### 5. 分层推理缓存 (Tiered Reasoning Cache)

| 层级 | 缓存内容 | 命中条件 |
|------|---------|---------|
| L1 | 精确查询结果 | 查询文本完全匹配 |
| L2 | 语义相似查询的推理路径 | 查询意图相似（LLM 判断） |
| L3 | 子路径模式缓存 | 某个节点的导航决策可复用 |

**价值**: 相同/相似查询几乎零成本。

### 6. 自适应 Token 预算 (Adaptive Token Budget)

根据查询复杂度动态分配 LLM 调用预算：

| 复杂度 | 策略 | LLM 调用 |
|--------|------|----------|
| 简单 | 直接定位 | 1 次 Pilot 调用 |
| 中等 | Beam Search | 2-4 次调用 |
| 复杂 | MCTS 深度探索 | 5+ 次调用 |

- 自动评估查询复杂度
- 预算耗尽时优雅降级（返回已有最佳结果）

**价值**: 不浪费 token，不牺牲质量。

---

## Tier 3: 智能 — "Deep Contextual Understanding"

### 7. 文档图谱 (Document Graph)

多文档间构建概念图谱：

- 共享术语、引用关系、矛盾检测
- 支持跨文档的统一检索（一个 query 横跨 N 个文档）
- 图谱感知的排序（关联文档的结果互相增强）

**价值**: 从单文档智能升级为知识库智能。

### 8. 上下文压缩 (Context Compression)

检索到的内容智能压缩：

- 保留关键信息，去除冗余
- 支持不同输出格式：extractive（原文摘录）和 abstractive（总结改写）
- 根据目标 LLM 的上下文窗口自动适配

**价值**: 无论文档多大，都能塞进 LLM 的上下文窗口。

### 9. 反馈学习 (Feedback Loop)

- 记录用户对检索结果的满意度信号（点击、采纳、拒绝）
- 基于反馈调整检索策略权重和 Pilot prompt
- 零成本的能力进化，无需重新训练

**价值**: 用得越多越准确。

---

## Tier 4: 开发者体验

### 10. 流式检索 (Streaming Retrieval)

- 检索过程流式返回中间结果
- 先返回高置信度结果，再逐步补充
- 适合上层构建实时响应的交互体验

**价值**: 用户感知延迟从"检索完成"降到"第一个结果返回"。

---

## 实施优先级

| 阶段 | 内容 | 依赖 |
|------|------|------|
| P0 | Reasoning Chain + Query Planner | 现有 Pilot + Retrieval |
| P1 | Reasoning Index + Adaptive Budget | 现有 Index Pipeline |
| P2 | Document Graph + Multi-hop | P0 + 多文档 Session |
| P3 | Tiered Cache + Context Compression | P1 |
| P4 | Streaming Retrieval | 现有 Client API |
| P5 | Feedback Loop | 全部基础能力就绪 |

**核心思路**: 先做推理链和查询规划（定义灵魂），再做预计算索引（定义性能），最后做生态增强。每一步都坚持 no vectors — 用结构推理替代向量相似度。
