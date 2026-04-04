# Pilot 设计文档

> Pilot - Retriever Pipeline 的大脑

## 概述

Pilot 是 Vectorless 检索系统的核心智能组件，负责理解查询、分析文档结构、做出搜索决策。与传统的向量检索不同，Pilot 使用 LLM 进行语义理解和导航决策，同时保持算法的高效执行。

### 设计哲学

```
┌─────────────────────────────────────────────────────────────────┐
│                    设计哲学                                      │
├─────────────────────────────────────────────────────────────────┤
│  1. 算法负责 "怎么走" - 高效、确定性、低延迟                        │
│  2. Pilot 负责 "去哪里" - 语义理解、歧义消解、方向判断               │
│  3. 关键决策点介入 - 不是每步都问 LLM，而是在需要时才问               │
│  4. 分层 fallback - LLM 失败时算法接管，算法失败时 Pilot 救援        │
└─────────────────────────────────────────────────────────────────┘
```

### 命名由来

**Pilot (驾驶员)** - 像飞机的驾驶员一样，Pilot 不直接操作每个机械部件（那是 Algorithm 的职责），而是负责：
- 理解目的地（用户查询）
- 规划航线（搜索策略）
- 在关键节点做决策（介入点）
- 应对突发情况（fallback）

---

## 1. Pilot 详细设计

### 1.1 整体架构

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                              Pilot 架构                                      │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  ┌───────────────────────────────────────────────────────────────────────┐  │
│  │                           Pilot (Core)                                 │  │
│  │                                                                       │  │
│  │   ┌─────────────┐   ┌─────────────┐   ┌─────────────┐                │  │
│  │   │   Query     │   │  Context    │   │  Decision   │                │  │
│  │   │  Analyzer   │──▶│   Builder   │──▶│   Engine    │                │  │
│  │   │  查询分析器  │   │  上下文构建  │   │   决策引擎   │                │  │
│  │   └─────────────┘   └─────────────┘   └──────┬──────┘                │  │
│  │                                              │                        │  │
│  │                                              ▼                        │  │
│  │   ┌─────────────┐   ┌─────────────┐   ┌─────────────┐                │  │
│  │   │   Response  │◀──│     LLM     │◀──│   Prompt    │                │  │
│  │   │   Parser    │   │   Client    │   │   Builder   │                │  │
│  │   │  响应解析器  │   │   客户端     │   │  提示词构建  │                │  │
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
│  │   │  预算控制器  │   │   降级管理   │   │  指标收集器  │                │  │
│  │   └─────────────┘   └─────────────┘   └─────────────┘                │  │
│  │                                                                       │  │
│  │   ┌─────────────┐   ┌─────────────┐   ┌─────────────┐                │  │
│  │   │   Policy    │   │    Cache    │   │   Logger    │                │  │
│  │   │   Manager   │   │  (Optional) │   │  (Tracing)  │                │  │
│  │   │  策略管理器  │   │    缓存      │   │   日志追踪   │                │  │
│  │   └─────────────┘   └─────────────┘   └─────────────┘                │  │
│  │                                                                       │  │
│  └───────────────────────────────────────────────────────────────────────┘  │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## 1.4 Pilot 决策的信息来源

Pilot 的决策依赖于多层信息，其中 TOC View 是核心——它就像导航电子地图。

### 信息来源架构

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                      Pilot 的"导航地图"                                      │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│                         ┌─────────────────┐                                │
│                         │   User Query    │                                │
│                         │   "PostgreSQL   │                                │
│                         │   连接池配置"    │                                │
│                         └────────┬────────┘                                │
│                                  │                                          │
│                                  ▼                                          │
│  ┌───────────────────────────────────────────────────────────────────────┐  │
│  │                        Pilot 上下文                                    │  │
│  │                                                                       │  │
│  │   ┌─────────────┐   ┌─────────────┐   ┌─────────────┐                │  │
│  │   │  TOC View   │   │ Current     │   │ Candidates  │                │  │
│  │   │  (电子地图)  │   │ Path        │   │ Info        │                │  │
│  │   │             │   │ (当前位置)   │   │ (候选路口)   │                │  │
│  │   └──────┬──────┘   └──────┬──────┘   └──────┬──────┘                │  │
│  │          │                 │                 │                        │  │
│  │          └─────────────────┼─────────────────┘                        │  │
│  │                            ▼                                          │  │
│  │                   ┌─────────────────┐                                 │  │
│  │                   │   LLM Decision  │                                 │  │
│  │                   │   (去哪里)       │                                 │  │
│  │                   └─────────────────┘                                 │  │
│  │                                                                       │  │
│  └───────────────────────────────────────────────────────────────────────┘  │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### TOC View - 电子地图（核心）

TOC View 是 Pilot 决策的核心依据，由 Index 阶段生成的内容构建：

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                          TOC View - 电子地图                                 │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  Index 阶段生成的内容:                                                       │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │  TreeNode {                                                         │   │
│  │    title: "配置",           // 标题                                  │   │
│  │    summary: "本章介绍...",   // LLM 生成的摘要 ← 关键！              │   │
│  │    depth: 1,                                                        │   │
│  │    children: [...],                                                 │   │
│  │  }                                                                  │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
│  TOC View 构建逻辑:                                                         │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │  generate_toc_view(tree, current_node):                             │   │
│  │                                                                     │   │
│  │    // 1. 从当前节点视角生成                                          │   │
│  │    // 2. 包含兄弟节点（横向视野）                                     │   │
│  │    // 3. 包含子节点（纵向视野）                                       │   │
│  │    // 4. 每个节点包含 title + summary                               │   │
│  │                                                                     │   │
│  │  输出示例:                                                          │   │
│  │  ┌─────────────────────────────────────────────────────────────┐   │   │
│  │  │  📍 当前位置: Root → 配置                                    │   │   │
│  │  │                                                             │   │   │
│  │  │  📂 兄弟节点:                                               │   │   │
│  │  │  ├─ 简介 [概述项目功能和架构]                                │   │   │
│  │  │  ├─ 安装 [安装步骤和环境要求]                                │   │   │
│  │  │  ├─ 配置 ⭐ [配置项详解]              ← 当前节点             │   │   │
│  │  │  │   ├─ 基本配置 [基础参数设置]                              │   │   │
│  │  │  │   ├─ 数据库配置 [数据库连接相关]  ← 关键匹配！            │   │   │
│  │  │  │   └─ 高级配置 [性能调优选项]                              │   │   │
│  │  │  └─ API 参考 [接口文档]                                     │   │   │
│  │  └─────────────────────────────────────────────────────────────┘   │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 三层信息结构

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                       Pilot 决策的三层信息                                   │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  Layer 1: TOC View (全局地图)                                               │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │  作用: 提供文档的全局结构视图                                         │   │
│  │  来源: Index Pipeline 的 Enrich 阶段生成的 summary                   │   │
│  │  Token: 约 200-500 tokens                                           │   │
│  │                                                                     │   │
│  │  示例:                                                              │   │
│  │  "本文档结构: 1.简介 2.安装 3.配置(3.1基本 3.2数据库 3.3高级) 4.API" │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
│  Layer 2: Current Path (当前位置)                                           │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │  作用: 告诉 LLM 我们已经走了哪里                                      │   │
│  │  来源: 搜索过程的路径记录                                            │   │
│  │  Token: 约 50-100 tokens                                            │   │
│  │                                                                     │   │
│  │  示例:                                                              │   │
│  │  "当前路径: Root → 配置 → 数据库配置"                               │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
│  Layer 3: Candidates Detail (候选路口详情)                                  │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │  作用: 提供候选节点的详细信息，供 LLM 判断                             │   │
│  │  来源: TreeNode 的 title + summary + 部分内容                       │   │
│  │  Token: 约 100-300 tokens                                           │   │
│  │                                                                     │   │
│  │  示例:                                                              │   │
│  │  候选节点:                                                          │   │
│  │  A. 连接字符串                                                      │   │
│  │     摘要: 配置数据库连接 URL 和认证信息                              │   │
│  │  B. 连接池 ⭐                                                       │   │
│  │     摘要: 配置连接池大小、超时、最大连接数等                          │   │
│  │  C. 超时设置                                                        │   │
│  │     摘要: 配置查询和连接超时时间                                     │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 决策过程示例

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                          Pilot 决策过程示例                                  │
└─────────────────────────────────────────────────────────────────────────────┘

Query: "PostgreSQL 连接池的最大连接数怎么配置？"

Step 1: 构建 TOC View (从 Index 阶段的 summary)
┌─────────────────────────────────────────────────────────────────────────────┐
│  TOC View (简化版):                                                         │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │  文档结构:                                                          │   │
│  │  1. 快速开始                                                        │   │
│  │  2. 配置                                                            │   │
│  │     2.1 基本配置                                                    │   │
│  │     2.2 数据库配置                                                  │   │
│  │         - 连接字符串                                                │   │
│  │         - 连接池 ← 包含"连接池"                                     │   │
│  │         - 超时设置                                                  │   │
│  │     2.3 高级配置                                                    │   │
│  │  3. API                                                            │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
│  这个 TOC 是 Index 阶段 LLM 生成的 summary 构成的！                          │
└─────────────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
Step 2: LLM 分析
┌─────────────────────────────────────────────────────────────────────────────┐
│  LLM 看到的信息:                                                            │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │  用户查询: "PostgreSQL 连接池的最大连接数怎么配置？"                  │   │
│  │                                                                     │   │
│  │  当前位置: 配置 → 数据库配置                                        │   │
│  │                                                                     │   │
│  │  候选节点:                                                          │   │
│  │  1. 连接字符串 [配置数据库 URL 和认证]                               │   │
│  │  2. 连接池 [配置池大小、超时、最大连接数]  ← 直接匹配！              │   │
│  │  3. 超时设置 [配置查询超时时间]                                     │   │
│  │                                                                     │   │
│  │  请判断哪个节点最可能包含答案？                                      │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
│  LLM 推理:                                                                  │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │  查询关键词: "连接池", "最大连接数"                                  │   │
│  │  候选 2 的摘要包含: "连接池", "最大连接数"                           │   │
│  │  → 候选 2 直接匹配，置信度 0.95                                     │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
Step 3: 返回决策
┌─────────────────────────────────────────────────────────────────────────────┐
│  PilotDecision {                                                           │
│    ranked_candidates: [                                                    │
│      (Node 2 "连接池", score: 0.95, reason: "摘要直接匹配查询关键词"),      │   │
│      (Node 3 "超时设置", score: 0.30, reason: "不太相关"),                  │   │
│      (Node 1 "连接字符串", score: 0.20, reason: "不相关"),                  │   │
│    ],                                                                      │
│    direction: GoDeeper,                                                    │
│    confidence: 0.95,                                                       │
│    reasoning: "候选节点'连接池'的摘要明确提到'最大连接数'，直接匹配查询",   │   │
│  }                                                                         │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 关键洞察

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                          关键洞察                                            │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  1. Index 阶段的 summary 质量决定 Pilot 效果                                 │
│     ┌─────────────────────────────────────────────────────────────────┐    │
│     │  好的 summary: "配置连接池大小、超时、最大连接数等参数"           │    │
│     │  差的 summary: "本章介绍连接池相关内容"                          │    │
│     │                                                                 │    │
│     │  → Index Enrich 阶段的 prompt 很重要！                          │    │
│     └─────────────────────────────────────────────────────────────────┘    │
│                                                                             │
│  2. TOC View 需要动态生成                                                   │
│     ┌─────────────────────────────────────────────────────────────────┐    │
│     │  不是整个文档的 TOC，而是从"当前节点"视角的局部视图               │    │
│     │  包含: 兄弟节点 + 子节点 + 父节点链                              │    │
│     │                                                                 │    │
│     │  这样 Token 消耗可控，且有上下文                                 │    │
│     └─────────────────────────────────────────────────────────────────┘    │
│                                                                             │
│  3. 类比: 高德地图导航                                                      │
│     ┌─────────────────────────────────────────────────────────────────┐    │
│     │  TOC View     = 地图 (道路网络)                                  │    │
│     │  Summary      = 路标 (路口描述)                                  │    │
│     │  Current Path = GPS 定位 (当前位置)                              │    │
│     │  Candidates   = 前方路口 (可选方向)                              │    │
│     │  Query        = 目的地 (要去哪里)                                │    │
│     │                                                                 │    │
│     │  Pilot        = 驾驶员 (综合以上信息做决策)                      │    │
│     └─────────────────────────────────────────────────────────────────┘    │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### ContextBuilder Token 预算分配

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                    ContextBuilder - Token 预算分配                           │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  Token 预算分配 (假设 500 tokens 总预算):                                    │
│                                                                             │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │                                                                     │   │
│  │  ┌────────────────────────────────────────┐  30% (150 tokens)      │   │
│  │  │  Query + Intent                        │                        │   │
│  │  │  "PostgreSQL 连接池最大连接数配置"      │                        │   │
│  │  └────────────────────────────────────────┘                        │   │
│  │                                                                     │   │
│  │  ┌────────────────────────────────────────┐  20% (100 tokens)      │   │
│  │  │  Current Path                          │                        │   │
│  │  │  Root → 配置 → 数据库配置              │                        │   │
│  │  └────────────────────────────────────────┘                        │   │
│  │                                                                     │   │
│  │  ┌────────────────────────────────────────┐  40% (200 tokens)      │   │
│  │  │  Candidates (title + summary each)     │                        │   │
│  │  │  A. 连接字符串 [配置URL和认证]         │                        │   │
│  │  │  B. 连接池 [配置池大小、最大连接数]     │                        │   │
│  │  │  C. 超时设置 [配置超时时间]            │                        │   │
│  │  └────────────────────────────────────────┘                        │   │
│  │                                                                     │   │
│  │  ┌────────────────────────────────────────┐  10% (50 tokens)       │   │
│  │  │  Sibling Context (兄弟节点概览)        │                        │   │
│  │  │  同级还有: 基本配置、高级配置          │                        │   │
│  │  └────────────────────────────────────────┘                        │   │
│  │                                                                     │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
│  动态调整策略:                                                              │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │  if candidates.len() > 5:                                           │   │
│  │      // 候选太多，减少每个候选的 detail                              │   │
│  │      只包含 title，不包含 summary                                   │   │
│  │                                                                     │   │
│  │  if depth > 3:                                                      │   │
│  │      // 深层搜索，减少 TOC 范围                                      │   │
│  │      只显示当前层和子层                                              │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## 2. 介入点详细设计

### 2.1 介入点类型

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                          Pilot 介入点                                        │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │  START - 搜索开始                                                    │   │
│  ├─────────────────────────────────────────────────────────────────────┤   │
│  │  时机: 搜索算法开始前                                                 │   │
│  │  任务: 理解查询意图，确定搜索起点和优先方向                             │   │
│  │  输入: query, tree (ToC view)                                        │   │
│  │  输出: entry_points, initial_direction, confidence                   │   │
│  │  配置: guide_at_start: bool                                          │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │  FORK - 分叉路口                                                     │   │
│  ├─────────────────────────────────────────────────────────────────────┤   │
│  │  时机: 当前节点有多个候选子节点时                                      │   │
│  │  任务: 判断哪个分支更可能包含答案                                      │   │
│  │  输入: path, candidates, query                                       │   │
│  │  输出: ranked_candidates, direction, confidence                      │   │
│  │  触发条件: candidates.len() > fork_threshold                         │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │  BACKTRACK - 回溯                                                    │   │
│  ├─────────────────────────────────────────────────────────────────────┤   │
│  │  时机: Judge 判断内容不足，需要回溯时                                  │   │
│  │  任务: 分析失败原因，建议新的搜索方向                                   │   │
│  │  输入: failed_path, visited, query                                   │   │
│  │  输出: alternative_branches, backtrack_reason                        │   │
│  │  配置: guide_at_backtrack: bool                                      │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │  EVALUATE - 节点评估                                                  │   │
│  ├─────────────────────────────────────────────────────────────────────┤   │
│  │  时机: 需要判断当前节点是否包含答案时                                   │   │
│  │  任务: 评估节点内容与查询的相关性                                       │   │
│  │  输入: node_content, query                                           │   │
│  │  输出: relevance_score, is_answer, reasoning                         │   │
│  │  触发条件: 到达叶子节点或算法不确定时                                   │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 2.2 介入判断逻辑

```rust
impl Pilot for LlmPilot {
    fn should_intervene(&self, state: &SearchState<'_>) -> bool {
        let config = &self.config.intervention;
        
        // 条件 1: 预算检查（最高优先级）
        if !self.budget.can_call() {
            return false;
        }
        
        // 条件 2: 候选数量超过阈值（分叉路口）
        if state.candidates.len() > config.fork_threshold {
            return true;
        }
        
        // 条件 3: 候选分数接近（算法无法区分）
        if self.scores_are_close(state.candidates, state.tree, config.score_gap_threshold) {
            return true;
        }
        
        // 条件 4: 当前分数过低（可能走错方向）
        if state.best_score < config.low_score_threshold {
            return true;
        }
        
        // 条件 5: 回溯时且配置允许
        if state.is_backtracking && self.config.guide_at_backtrack {
            return true;
        }
        
        // 条件 6: 每层介入次数限制
        let level_calls = self.get_level_calls(state.depth);
        if level_calls >= config.max_interventions_per_level {
            return false;
        }
        
        false
    }
}

/// 判断候选分数是否接近
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

### 2.3 介入配置

```rust
/// 介入配置
#[derive(Debug, Clone)]
pub struct InterventionConfig {
    /// 候选数量阈值（超过此值考虑介入）
    pub fork_threshold: usize,
    /// 分数差距阈值（差距小于此值时介入）
    pub score_gap_threshold: f32,
    /// 低分阈值（最高分低于此值时介入）
    pub low_score_threshold: f32,
    /// 每层最大介入次数
    pub max_interventions_per_level: usize,
}

impl Default for InterventionConfig {
    fn default() -> Self {
        Self {
            fork_threshold: 3,           // 3 个以上候选时介入
            score_gap_threshold: 0.15,   // 分数差距 < 0.15 时介入
            low_score_threshold: 0.3,    // 分数 < 0.3 时介入
            max_interventions_per_level: 2,  // 每层最多介入 2 次
        }
    }
}
```

---

## 3. Fallback 机制

### 3.1 降级层级

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                          Fallback 降级层级                                   │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  Level 0: 正常 LLM 调用                                                      │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │  条件: 预算充足，LLM 服务可用                                         │   │
│  │  行为: 正常调用 LLM，获取决策                                          │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                          │ 失败                                             │
│                          ▼                                                  │
│  Level 1: 重试                                                              │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │  条件: 网络错误、超时、rate limit                                     │   │
│  │  行为: 指数退避重试，最多 3 次                                         │   │
│  │  参数: initial_delay=1s, max_delay=10s, max_attempts=3              │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                          │ 失败                                             │
│                          ▼                                                  │
│  Level 2: 简化上下文                                                        │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │  条件: token 超限、上下文过长                                         │   │
│  │  行为: 减少上下文信息，只保留核心内容                                   │   │
│  │  策略: 移除 ToC，只保留当前节点和候选标题                               │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                          │ 失败                                             │
│                          ▼                                                  │
│  Level 3: 纯算法模式                                                        │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │  条件: LLM 完全不可用、预算耗尽                                        │   │
│  │  行为: 完全依赖算法打分，不调用 LLM                                     │   │
│  │  结果: 使用 NodeScorer 的关键词匹配                                    │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 3.2 Fallback 策略定义

```rust
/// 降级策略
#[derive(Debug, Clone)]
pub enum FallbackStrategy {
    /// 重试策略
    Retry {
        max_attempts: usize,
        backoff: BackoffPolicy,
    },
    /// 简化上下文
    SimplifyContext {
        remove_toc: bool,
        max_candidates: usize,
    },
    /// 使用算法替代
    UseAlgorithm,
    /// 返回默认决策
    ReturnDefault,
}

/// 退避策略
#[derive(Debug, Clone)]
pub enum BackoffPolicy {
    /// 固定间隔
    Fixed { delay_ms: u64 },
    /// 线性增长
    Linear { initial_ms: u64, increment_ms: u64 },
    /// 指数增长
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

### 3.3 FallbackManager 实现

```rust
/// 降级管理器
pub struct FallbackManager {
    config: FallbackConfig,
    /// 当前降级级别
    current_level: AtomicU8,
    /// 连续失败次数
    consecutive_failures: AtomicUsize,
}

impl FallbackManager {
    /// 执行带降级的调用
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
                    // Level 0: 正常调用
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
                    // Level 1: 重试
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
                    // Level 2: 简化上下文
                    // 由调用方处理，返回特定错误
                    return Err(FallbackError::SimplifyContextRequired);
                }
                3 => {
                    // Level 3: 纯算法模式
                    return Err(FallbackError::AlgorithmFallback);
                }
                _ => unreachable!(),
            }
        }
    }
    
    /// 重试操作
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
        // 逐渐恢复到更高级别
        let current = self.current_level.load(Ordering::Relaxed);
        if current > 0 {
            self.current_level.fetch_sub(1, Ordering::Relaxed);
        }
    }
    
    fn on_failure(&self) {
        let failures = self.consecutive_failures.fetch_add(1, Ordering::Relaxed);
        // 连续失败 3 次后升级降级级别
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

## 4. Token 消耗衡量

### 4.1 预算配置

```rust
/// 预算配置
#[derive(Debug, Clone)]
pub struct BudgetConfig {
    /// 单次检索最大 token 数
    pub max_tokens_per_query: usize,
    /// 单次 LLM 调用最大 token 数
    pub max_tokens_per_call: usize,
    /// 单次检索最大 LLM 调用次数
    pub max_calls_per_query: usize,
    /// 每层（深度）最大调用次数
    pub max_calls_per_level: usize,
    /// 是否硬性限制（true: 超预算直接拒绝；false: 尝试继续）
    pub hard_limit: bool,
}

impl Default for BudgetConfig {
    fn default() -> Self {
        Self {
            max_tokens_per_query: 2000,   // 单次检索最多 2000 tokens
            max_tokens_per_call: 500,     // 单次调用最多 500 tokens
            max_calls_per_query: 5,       // 最多调用 5 次
            max_calls_per_level: 2,       // 每层最多 2 次
            hard_limit: true,
        }
    }
}
```

### 4.2 预算控制器

```rust
/// 预算控制器
pub struct BudgetController {
    config: BudgetConfig,
    /// 已使用的 token 数
    tokens_used: AtomicUsize,
    /// 已调用的次数
    calls_made: AtomicUsize,
    /// 每层调用次数
    level_calls: RwLock<HashMap<usize, usize>>,
}

impl BudgetController {
    /// 创建新的预算控制器
    pub fn new(config: BudgetConfig) -> Self {
        Self {
            config,
            tokens_used: AtomicUsize::new(0),
            calls_made: AtomicUsize::new(0),
            level_calls: RwLock::new(HashMap::new()),
        }
    }
    
    /// 检查是否可以调用 LLM
    pub fn can_call(&self) -> bool {
        let calls = self.calls_made.load(Ordering::Relaxed);
        let tokens = self.tokens_used.load(Ordering::Relaxed);
        
        calls < self.config.max_calls_per_query
            && tokens < self.config.max_tokens_per_query
    }
    
    /// 检查特定层是否可以调用
    pub fn can_call_at_level(&self, level: usize) -> bool {
        if !self.can_call() {
            return false;
        }
        
        let level_calls = self.level_calls.read().unwrap();
        let calls = level_calls.get(&level).copied().unwrap_or(0);
        calls < self.config.max_calls_per_level
    }
    
    /// 预估调用成本
    pub fn estimate_cost(&self, context: &str) -> usize {
        // 使用 tiktoken 或简单的字符估算
        // 粗略估算：1 token ≈ 4 chars (英文) 或 1.5 chars (中文)
        let char_count = context.chars().count();
        // 保守估计，按中文计算
        char_count / 2 + 100  // +100 为输出预留
    }
    
    /// 检查预估成本是否在预算内
    pub fn can_afford(&self, estimated_cost: usize) -> bool {
        let remaining = self.remaining_budget();
        estimated_cost <= remaining && estimated_cost <= self.config.max_tokens_per_call
    }
    
    /// 获取剩余预算
    pub fn remaining_budget(&self) -> usize {
        let used = self.tokens_used.load(Ordering::Relaxed);
        self.config.max_tokens_per_query.saturating_sub(used)
    }
    
    /// 记录 token 使用
    pub fn record_usage(&self, input_tokens: usize, output_tokens: usize, level: usize) {
        let total = input_tokens + output_tokens;
        self.tokens_used.fetch_add(total, Ordering::Relaxed);
        self.calls_made.fetch_add(1, Ordering::Relaxed);
        
        // 记录层级调用
        let mut level_calls = self.level_calls.write().unwrap();
        *level_calls.entry(level).or_insert(0) += 1;
    }
    
    /// 获取使用统计
    pub fn get_usage_stats(&self) -> BudgetUsage {
        BudgetUsage {
            tokens_used: self.tokens_used.load(Ordering::Relaxed),
            calls_made: self.calls_made.load(Ordering::Relaxed),
            max_tokens: self.config.max_tokens_per_query,
            max_calls: self.config.max_calls_per_query,
        }
    }
    
    /// 重置（新查询开始时）
    pub fn reset(&self) {
        self.tokens_used.store(0, Ordering::Relaxed);
        self.calls_made.store(0, Ordering::Relaxed);
        self.level_calls.write().unwrap().clear();
    }
}

/// 预算使用统计
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

### 4.3 Token 消耗流程

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                        Token 消耗流程                                        │
└─────────────────────────────────────────────────────────────────────────────┘

LLM 调用前:
┌─────────────────────────────────────────────────────────────────────────────┐
│  1. BudgetController.can_call()                                             │
│     └─ 检查: calls_made < max_calls_per_query                               │
│     └─ 检查: tokens_used < max_tokens_per_query                             │
│                                                                             │
│  2. BudgetController.can_call_at_level(depth)                               │
│     └─ 检查: level_calls[depth] < max_calls_per_level                       │
│                                                                             │
│  3. BudgetController.estimate_cost(context)                                 │
│     └─ 预估: input_tokens + output_tokens (预留)                            │
│                                                                             │
│  4. BudgetController.can_afford(estimated_cost)                             │
│     └─ 检查: estimated_cost <= remaining_budget                             │
│     └─ 检查: estimated_cost <= max_tokens_per_call                          │
│                                                                             │
│  决策: 全部通过 → 继续调用；任一失败 → 跳过或降级                              │
└─────────────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
LLM 调用:
┌─────────────────────────────────────────────────────────────────────────────┐
│  LLM Client 返回:                                                           │
│  - usage.prompt_tokens (输入 tokens)                                        │
│  - usage.completion_tokens (输出 tokens)                                    │
└─────────────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
LLM 调用后:
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

## 5. 职责划分

### 5.1 模块职责

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                          Pilot 模块职责划分                                  │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │  QueryAnalyzer - 查询分析器                                          │   │
│  ├─────────────────────────────────────────────────────────────────────┤   │
│  │  职责:                                                              │   │
│  │  • 分析查询复杂度（简单/中等/复杂）                                   │   │
│  │  • 提取关键词和实体                                                  │   │
│  │  • 识别查询意图（事实查询/对比/解释/操作指南）                         │   │
│  │  • 判断是否需要 Pilot 介入                                           │   │
│  │                                                                     │   │
│  │  输入: query: String                                                │   │
│  │  输出: QueryAnalysis { complexity, keywords, intent, needs_pilot }  │   │
│  │                                                                     │   │
│  │  实现策略:                                                          │   │
│  │  • 轻量级：基于规则（关键词计数、句子结构）                            │   │
│  │  • 重量级：LLM 分析（复杂查询）                                       │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │  ContextBuilder - 上下文构建器                                       │   │
│  ├─────────────────────────────────────────────────────────────────────┤   │
│  │  职责:                                                              │   │
│  │  • 构建发送给 LLM 的上下文信息                                       │   │
│  │  • 提取当前路径的节点信息（标题、摘要、深度）                          │   │
│  │  • 构建候选节点的描述                                                │   │
│  │  • 生成 ToC 视图（从当前节点视角）                                    │   │
│  │  • 控制 token 预算分配                                               │   │
│  │                                                                     │   │
│  │  输入: tree, path, candidates, query                                │   │
│  │  输出: PilotContext { path_info, candidates_info, toc_view }        │   │
│  │                                                                     │   │
│  │  Token 预算分配:                                                    │   │
│  │  • path_info: 20%                                                   │   │
│  │  • candidates_info: 50%                                             │   │
│  │  • toc_view: 30%                                                    │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │  PromptBuilder - 提示词构建器                                        │   │
│  ├─────────────────────────────────────────────────────────────────────┤   │
│  │  职责:                                                              │   │
│  │  • 根据场景选择合适的 prompt 模板                                    │   │
│  │  • 填充模板变量                                                      │   │
│  │  • 管理 system prompt 和 user prompt                                │   │
│  │  • 支持多语言                                                        │   │
│  │                                                                     │   │
│  │  场景类型:                                                          │   │
│  │  • START: 搜索开始，确定起点                                        │   │
│  │  • FORK: 分叉路口，选择分支                                         │   │
│  │  • BACKTRACK: 回溯时，分析失败原因                                   │   │
│  │  • EVALUATE: 评估节点是否包含答案                                    │   │
│  │                                                                     │   │
│  │  设计要点:                                                          │   │
│  │  • 模板可配置（用户可自定义）                                        │   │
│  │  • 包含 few-shot 示例（提高质量）                                    │   │
│  │  • 输出格式明确（JSON schema）                                       │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │  DecisionEngine - 决策引擎                                           │   │
│  ├─────────────────────────────────────────────────────────────────────┤   │
│  │  职责:                                                              │   │
│  │  • 判断何时需要调用 LLM（should_intervene）                           │   │
│  │  • 协调 LLM 调用                                                    │   │
│  │  • 融合算法打分和 LLM 建议                                          │   │
│  │  • 做出最终决策                                                      │   │
│  │                                                                     │   │
│  │  决策逻辑:                                                          │   │
│  │  ┌─────────────────────────────────────────────────────────────┐    │   │
│  │  │  should_intervene(state) -> bool                             │    │   │
│  │  │                                                              │    │   │
│  │  │  // 策略 1: 分叉路口                                          │    │   │
│  │  │  if candidates.len() > config.fork_threshold { return true } │    │   │
│  │  │                                                              │    │   │
│  │  │  // 策略 2: 算法不确定                                        │    │   │
│  │  │  if scores_are_close(candidates) { return true }             │    │   │
│  │  │                                                              │    │   │
│  │  │  // 策略 3: 低置信度                                          │    │   │
│  │  │  if best_score < config.low_confidence_threshold { return true }│  │
│  │  │                                                              │    │   │
│  │  │  // 策略 4: 预算检查                                          │    │   │
│  │  │  if budget_exhausted() { return false }                      │    │   │
│  │  │                                                              │    │   │
│  │  │  return false                                                │    │   │
│  │  └─────────────────────────────────────────────────────────────┘    │   │
│  │                                                                     │   │
│  │  融合逻辑:                                                          │   │
│  │  ┌─────────────────────────────────────────────────────────────┐    │   │
│  │  │  final_score = α * algo_score + β * llm_confidence          │    │   │
│  │  │                                                              │    │   │
│  │  │  // α 和 β 根据场景动态调整                                   │    │   │
│  │  │  // - LLM 高置信度时 β 更高                                   │    │   │
│  │  │  // - 算法高分且 LLM 低置信度时 α 更高                         │    │   │
│  │  └─────────────────────────────────────────────────────────────┘    │   │
│  │                                                                     │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │  ResponseParser - 响应解析器                                         │   │
│  ├─────────────────────────────────────────────────────────────────────┤   │
│  │  职责:                                                              │   │
│  │  • 解析 LLM 返回的 JSON                                             │   │
│  │  • 处理格式错误                                                      │   │
│  │  • 提取结构化信息（ranked_candidates, direction, confidence）         │   │
│  │  • 验证响应有效性                                                    │   │
│  │                                                                     │   │
│  │  解析策略:                                                          │   │
│  │  ┌─────────────────────────────────────────────────────────────┐    │   │
│  │  │  parse(response: String) -> Result<PilotDecision>            │    │   │
│  │  │                                                              │    │   │
│  │  │  // 优先级 1: JSON 解析                                       │    │   │
│  │  │  if let Ok(json) = parse_json(response) { return json }      │    │   │
│  │  │                                                              │    │   │
│  │  │  // 优先级 2: 正则提取                                        │    │   │
│  │  │  if let Some(data) = extract_by_regex(response) { return data }│   │
│  │  │                                                              │    │   │
│  │  │  // 优先级 3: 默认值                                          │    │   │
│  │  │  return PilotDecision::default()                             │    │   │
│  │  └─────────────────────────────────────────────────────────────┘    │   │
│  │                                                                     │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
│                                                                             │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │  BudgetController - 预算控制器                                       │   │
│  ├─────────────────────────────────────────────────────────────────────┤   │
│  │  职责:                                                              │   │
│  │  • 追踪 token 消耗                                                  │   │
│  │  • 控制 LLM 调用次数                                                │   │
│  │  • 预估调用成本                                                     │   │
│  │  • 强制执行预算限制     
│  │  │                                                              │    │   │                                            │   │
│  │  配置:                                                              │   │
│  │  ┌─────────────────────────────────────────────────────────────┐    │   │
│  │  │  BudgetConfig {                                             │    │   │
│  │  │    max_tokens_per_query: usize,    // 单次检索总预算         │    │   │
│  │  │    max_tokens_per_call: usize,     // 单次调用预算           │    │   │
│  │  │    max_calls_per_query: usize,     // 最大调用次数           │    │   │
│  │  │    max_calls_per_level: usize,     // 每层最大调用           │    │   │
│  │  │    hard_limit: bool,               // 是否硬性限制           │    │   │
│  │  │  }                                                          │    │   │
│  │  └─────────────────────────────────────────────────────────────┘    │   │
│  │  接口:                                                              │   │
│  │  • can_call() -> bool                                               │   │
│  │  • can_call_at_level(level) -> bool                                 │   │
│  │  • estimate_cost(context) -> usize                                  │   │
│  │  • can_afford(estimated_cost) -> bool                               │   │
│  │  • record_usage(input, output, level)                               │   │
│  │  • remaining_budget() -> usize                                      │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │  FallbackManager - 降级管理器                                        │   │
│  ├─────────────────────────────────────────────────────────────────────┤   │
│  │  职责:                                                              │   │
│  │  • 处理 LLM 调用失败                                                │   │
│  │  • 提供降级策略                                                     │   │
│  │  • 记录失败原因                                                     │   │
│  │  • 自动恢复机制                                                     │   │
│  │                                                                     │   │
│  │  降级层级:                                                          │   │
│  │  ┌─────────────────────────────────────────────────────────────┐    │   │
│  │  │  Level 0: 正常 LLM 调用                                     │    │   │
│  │  │     ↓ 失败                                                  │    │   │
│  │  │  Level 1: 重试 (最多 3 次，指数退避)                         │    │   │
│  │  │     ↓ 失败                                                  │    │   │
│  │  │  Level 2: 简化 prompt (减少上下文)                          │    │   │
│  │  │     ↓ 失败                                                  │    │   │
│  │  │  Level 3: 纯算法模式 (完全降级)                              │    │   │
│  │  └─────────────────────────────────────────────────────────────┘    │   │
│  │                                                                     │   │
│  │  降级策略:                                                          │   │
│  │  ┌─────────────────────────────────────────────────────────────┐    │   │
│  │  │  enum FallbackStrategy {                                    │    │   │
│  │  │    Retry { max_attempts: usize, backoff: BackoffPolicy },   │    │   │
│  │  │    SimplifyContext,  // 减少上下文信息                       │    │   │
│  │  │    UseAlgorithm,     // 使用算法打分                         │    │   │
│  │  │    ReturnDefault,    // 返回默认决策                         │    │   │
│  │  │  }                                                          │    │   │
│  │  └─────────────────────────────────────────────────────────────┘    │   │
│  │                                                                     │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │  PolicyManager - 策略管理器                                          │   │
│  ├─────────────────────────────────────────────────────────────────────┤   │
│  │  职责:                                                              │   │
│  │  • 管理介入策略配置                                                 │   │
│  │  • 支持多种运行模式                                                 │   │
│  │  • 动态调整参数（可选）                                              │   │
│  │                                                                     │   │
│  │  策略模式:                                                          │   │
│  │  ┌─────────────────────────────────────────────────────────────┐    │   │
│  │  │  enum PilotMode {                                           │    │   │
│  │  │    Aggressive,   // 激进模式：频繁调用 LLM                   │    │   │
│  │  │    Balanced,     // 平衡模式：按需调用 (默认)                 │    │   │
│  │  │    Conservative, // 保守模式：尽量少调用                     │    │   │
│  │  │    AlgorithmOnly,// 纯算法模式：不调用 LLM                   │    │   │
│  │  │  }                                                          │    │   │
│  │  └─────────────────────────────────────────────────────────────┘    │   │
│  │                                                                     │   │
│  │  参数调整:                                                          │   │
│  │  ┌─────────────────────────────────────────────────────────────┐    │   │
│  │  │  // 根据历史效果动态调整                                      │    │   │
│  │  │  fn adjust_threshold(&mut self, performance: &PerformanceMetrics) {│  │
│  │  │    // 如果 LLM 建议准确率高，降低介入阈值                     │    │   │
│  │  │    if performance.llm_accuracy > 0.8 {                      │    │   │
│  │  │      self.fork_threshold = 2;                               │    │   │
│  │  │    }                                                        │    │   │
│  │  │  }                                                          │    │   │
│  │  └─────────────────────────────────────────────────────────────┘    │   │
│  │                                                                     │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │  MetricsCollector - 指标收集器                                       │   │
│  ├─────────────────────────────────────────────────────────────────────┤   │
│  │  职责:                                                              │   │
│  │  • 收集性能指标                                                     │   │
│  │  • 追踪 LLM 调用详情                                                │   │
│  │  • 计算成本                                                        │   │
│  │  • 支持可观测性                                                     │   │
│  │                                                                     │   │
│  │  指标类型:                                                          │   │
│  │  ┌─────────────────────────────────────────────────────────────┐    │   │
│  │  │  PilotMetrics {                                             │    │   │
│  │  │    // 调用统计                                               │    │   │
│  │  │    total_calls: usize,                                      │    │   │
│  │  │    successful_calls: usize,                                 │    │   │
│  │  │    failed_calls: usize,                                     │    │   │
│  │  │    fallback_count: usize,                                   │    │   │
│  │  │                                                             │    │   │
│  │  │    // Token 统计                                             │    │   │
│  │  │    total_input_tokens: usize,                               │    │   │
│  │  │    total_output_tokens: usize,                              │    │   │
│  │  │    avg_tokens_per_call: f64,                                │    │   │
│  │  │                                                             │    │   │
│  │  │    // 延迟统计                                               │    │   │
│  │  │    total_latency_ms: u64,                                   │    │   │
│  │  │    avg_latency_ms: f64,                                     │    │   │
│  │  │    p50_latency_ms: u64,                                     │    │   │
│  │  │    p99_latency_ms: u64,                                     │    │   │
│  │  │                                                             │    │   │
│  │  │    // 效果统计 (需要反馈)                                     │    │   │
│  │  │    llm_decision_accuracy: Option<f64>,  // LLM 决策准确率    │    │   │
│  │  │    retrieval_precision: Option<f64>,     // 检索准确率       │    │   │
│  │  │  }                                                          │    │   │
│  │  └─────────────────────────────────────────────────────────────┘    │   │
│  │                                                                     │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 5.2 Pilot 与 Algorithm 的协作

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                      Pilot 与 Algorithm 协作关系                             │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │                    职责边界                                          │   │
│  ├─────────────────────────────────────────────────────────────────────┤   │
│  │                                                                     │   │
│  │  Pilot (大脑)                    Algorithm (手脚)                   │   │
│  │  ┌─────────────────────┐        ┌─────────────────────┐            │   │
│  │  │ • 理解查询意图       │        │ • 执行树遍历         │            │   │
│  │  │ • 分析文档结构       │        │ • 高效搜索路径       │            │   │
│  │  │ • 语义判断          │        │ • 计算节点分数       │            │   │
│  │  │ • 方向决策          │        │ • 管理搜索状态       │            │   │
│  │  │ • 歧义消解          │        │ • 返回搜索结果       │            │   │
│  │  └─────────────────────┘        └─────────────────────┘            │   │
│  │                                                                     │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │                    协作流程                                          │   │
│  ├─────────────────────────────────────────────────────────────────────┤   │
│  │                                                                     │   │
│  │  1. Algorithm 执行搜索                                              │   │
│  │     │                                                               │   │
│  │     ▼                                                               │   │
│  │  2. Algorithm 遇到决策点，询问 Pilot                                 │   │
│  │     │  Pilot.should_intervene(state)                                │   │
│  │     ▼                                                               │   │
│  │  3a. Pilot 返回 false → Algorithm 继续用自己的 scorer               │   │
│  │     │                                                               │   │
│  │  3b. Pilot 返回 true → Pilot.decide(state)                          │   │
│  │     │  │                                                            │   │
│  │     │  ▼                                                            │   │
│  │     │  Pilot 返回决策 → Algorithm 融合决策继续搜索                   │   │
│  │     │                                                               │   │
│  │     ▼                                                               │   │
│  │  4. 重复直到搜索完成                                                 │   │
│  │                                                                     │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## 6. Pilot 完整调用流程 

┌─────────────────────────────────────────────────────────────────────────────┐
│                        Pilot 完整调用流程                                    │
└─────────────────────────────────────────────────────────────────────────────┘

用户查询: "如何配置 PostgreSQL 连接池的最大连接数？"
                    │
                    ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│ Step 1: QueryAnalyzer 分析查询                                               │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  QueryAnalysis {                                                            │
│    complexity: Medium,           // 中等复杂度                               │
│    keywords: ["PostgreSQL", "连接池", "最大连接数", "配置"],                  │
│    intent: HowTo,               // 操作指南类                                │
│    needs_pilot: true,           // 需要 Pilot 介入                           │
│  }                                                                          │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
                    │
                    ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│ Step 2: Pilot.guide_start() - 搜索前指导                                     │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  BudgetController: 检查预算 (通过)                                           │
│                                                                             │
│  ContextBuilder:                                                            │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │  ToC View:                                                          │   │
│  │  1. 简介                                                            │   │
│  │  2. 安装                                                            │   │
│  │  3. 配置                                                            │   │
│  │     3.1 基本配置                                                    │   │
│  │     3.2 数据库配置                                                  │   │
│  │     3.3 高级配置                                                    │   │
│  │  4. API 参考                                                        │   │
│  │  ...                                                                │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
│  PromptBuilder: 构建 START 场景 prompt                                       │
│                                                                             │
│  LLM Response:                                                              │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │  {                                                                   │   │
│  │    "entry_points": ["配置", "数据库配置"],                           │   │
│  │    "reasoning": "查询关于数据库连接池配置，应从配置章节开始",         │   │
│  │    "confidence": 0.9                                                │   │
│  │  }                                                                   │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
│  MetricsCollector: 记录 (input: 150, output: 50, latency: 230ms)            │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
                    │
                    ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│ Step 3: BeamSearch 开始搜索                                                  │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  迭代 1: Root → [简介, 安装, 配置, API, ...]                                 │
│                                                                             │
│  Algorithm 打分:                                                            │
│    "配置" -> 0.75 (关键词匹配)                                               │
│    "API"   -> 0.35                                                          │
│    "安装"  -> 0.10                                                          │
│                                                                             │
│  Pilot.should_intervene():                                                  │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │  candidates.len() (6) > fork_threshold (3)  → true                  │   │
│  │  → 需要介入                                                         │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
│  Pilot.decide():                                                            │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │  LLM 分析:                                                          │   │
│  │  "查询明确指向配置相关内容，'配置'章节最相关"                          │   │
│  │                                                                     │   │
│  │  ranked_candidates: [                                               │   │
│  │    ("配置", 0.95, "明确提到配置"),                                   │   │
│  │    ("API", 0.40, "可能有相关 API"),                                 │   │
│  │  ]                                                                  │   │
│  │  confidence: 0.9                                                    │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
│  融合打分:                                                                   │
│    "配置" -> 0.75*0.4 + 0.95*0.6*0.9 = 0.84                                 │
│                                                                             │
│  选择: "配置" 节点深入                                                       │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
                    │
                    ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│ Step 4: 继续搜索 - 迭代 2                                                    │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  当前位置: Root → 配置                                                       │
│  候选: [基本配置, 数据库配置, 高级配置, 性能调优]                              │
│                                                                             │
│  Algorithm 打分:                                                            │
│    "数据库配置" -> 0.92 (强匹配!)                                            │
│    "高级配置"   -> 0.45                                                     │
│                                                                             │
│  Pilot.should_intervene():                                                  │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │  best_score (0.92) > low_score_threshold (0.3)  → OK               │   │
│  │  score_gap (0.47) > threshold (0.15)           → OK               │   │
│  │  → 不需要介入，算法很确定                                            │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
│  直接使用算法打分，选择 "数据库配置"                                          │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
                    │
                    ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│ Step 5: 继续搜索 - 迭代 3                                                    │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  当前位置: Root → 配置 → 数据库配置                                          │
│  候选: [连接字符串, 连接池, 超时设置, SSL配置]                                 │
│                                                                             │
│  Algorithm 打分:                                                            │
│    "连接池" -> 0.98 (完美匹配!)                                              │
│                                                                             │
│  → 找到目标，搜索结束                                                        │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
                    │
                    ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│ Step 6: 返回结果                                                             │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  SearchResult {                                                             │
│    path: [Root → 配置 → 数据库配置 → 连接池],                                │
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


7. 代码结构

```
src/retrieval/
├── mod.rs
├── pilot/                      # Pilot 模块
│   ├── mod.rs                  # 模块入口
│   ├── trait.rs                # Pilot trait 定义
│   ├── config.rs               # 配置类型（PilotConfig, BudgetConfig, InterventionConfig）
│   ├── decision.rs             # 决策类型（PilotDecision, SearchDirection）
│   ├── analyzer.rs             # QueryAnalyzer
│   ├── builder.rs              # ContextBuilder
│   ├── engine.rs               # DecisionEngine
│   ├── parser.rs               # ResponseParser
│   ├── policy.rs               # PolicyManager
│   ├── budget.rs               # BudgetController
│   ├── fallback.rs             # FallbackManager
│   ├── metrics.rs              # MetricsCollector
│   ├── llm_pilot.rs            # LlmPilot 实现
│   ├── noop_pilot.rs           # NoopPilot 实现（空实现，用于纯算法模式）
│   └── prompts/                # Prompt 模板
│       ├── mod.rs
│       ├── start.rs            # START 场景模板
│       ├── fork.rs             # FORK 场景模板
│       ├── backtrack.rs        # BACKTRACK 场景模板
│       └── evaluate.rs         # EVALUATE 场景模板
├── search/
│   ├── mod.rs
│   ├── trait.rs                # SearchTree trait（修改：增加 pilot 参数）
│   ├── scorer.rs               # NodeScorer（现有）
│   ├── beam.rs                 # BeamSearch（修改：集成 Pilot）
│   ├── greedy.rs               # GreedySearch（修改：集成 Pilot）
│   └── mcts.rs                 # MctsSearch（修改：集成 Pilot）
├── stages/
│   ├── search.rs               # SearchStage（修改：注入 Pilot）
│   └── ...
└── ...
```

---

## 7. 配置示例

```rust
// 默认配置
let config = PilotConfig {
    mode: PilotMode::Balanced,
    budget: BudgetConfig::default(),
    intervention: InterventionConfig::default(),
    guide_at_start: true,
    guide_at_backtrack: true,
    prompt_template_path: None,
};

// 高质量模式（更多 LLM 调用）
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

// 低成本模式（最少 LLM 调用）
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

// 纯算法模式（不调用 LLM）
let algorithm_only_config = PilotConfig {
    mode: PilotMode::AlgorithmOnly,
    ..Default::default()
};
```

---

## 8. 使用示例

```rust
use vectorless::retrieval::pilot::{LlmPilot, PilotConfig, PilotMode};
use vectorless::retrieval::search::BeamSearch;
use vectorless::llm::LlmClient;

// 创建 Pilot
let llm_client = LlmClient::from_env()?;
let pilot = LlmPilot::new(llm_client, PilotConfig::default());

// 创建搜索引擎（注入 Pilot）
let search = BeamSearch::new().with_pilot(pilot);

// 执行搜索
let result = search.search(&tree, &context, &config).await?;

// 查看指标
println!("LLM calls: {}", result.metrics.llm_calls);
println!("Tokens used: {}", result.metrics.tokens_used);
println!("Avg latency: {}ms", result.metrics.avg_latency_ms);
```
