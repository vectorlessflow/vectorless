# Feedback Learning 设计文档

> Pilot 反馈学习系统 - 从用户反馈中持续改进决策

## 概述

Feedback Learning 是 Pilot 的学习子系统，通过收集用户对检索结果的反馈，持续优化 Pilot 的决策能力。系统会追踪不同场景下的决策准确性，并据此调整后续决策的置信度和策略。

### 设计目标

```
┌─────────────────────────────────────────────────────────────────┐
│                      设计目标                                    │
├─────────────────────────────────────────────────────────────────┤
│  1. 收集反馈 - 记录用户对检索结果的评价                            │
│  2. 学习模式 - 识别在哪些场景下 Pilot 表现好/差                    │
│  3. 调整决策 - 根据历史表现调整置信度和策略                         │
│  4. 持续改进 - 随着数据积累，决策质量逐步提升                       │
└─────────────────────────────────────────────────────────────────┘
```

---

## 1. 整体架构

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                      Feedback Learning 系统架构                              │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  ┌───────────────────────────────────────────────────────────────────────┐  │
│  │                        数据流                                          │  │
│  │                                                                       │  │
│  │   检索完成                                                            │  │
│  │      │                                                                │  │
│  │      ▼                                                                │  │
│  │   ┌─────────────┐     ┌─────────────┐     ┌─────────────┐           │  │
│  │   │   Feedback  │────▶│  Feedback   │────▶│    Pilot    │           │  │
│  │   │   Record    │     │   Store     │     │   Learner   │           │  │
│  │   └─────────────┘     └─────────────┘     └──────┬──────┘           │  │
│  │                                                   │                   │  │
│  │                                                   ▼                   │  │
│  │                                          ┌─────────────┐              │  │
│  │                                          │  Decision   │              │  │
│  │                                          │ Adjustment  │              │  │
│  │                                          └─────────────┘              │  │
│  │                                                   │                   │  │
│  │                                                   ▼                   │  │
│  │                                          下次检索决策                  │  │
│  │                                                                       │  │
│  └───────────────────────────────────────────────────────────────────────┘  │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## 2. 核心组件

### 2.1 FeedbackRecord - 反馈记录

```rust
/// 反馈记录
pub struct FeedbackRecord {
    /// 唯一反馈 ID
    pub id: FeedbackId,
    /// 关联的决策 ID
    pub decision_id: DecisionId,
    /// 决策是否正确
    pub was_correct: bool,
    /// Pilot 当时的置信度
    pub pilot_confidence: f64,
    /// 介入点类型
    pub intervention_point: InterventionPoint,
    /// 查询哈希（用于聚合相似查询）
    pub query_hash: u64,
    /// 路径哈希（用于聚合相似路径）
    pub path_hash: u64,
    /// 时间戳
    pub timestamp_ms: u64,
    /// 可选的用户评论
    pub comment: Option<String>,
}
```

### 2.2 FeedbackStore - 反馈存储

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                          FeedbackStore 架构                                  │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │  FeedbackStore                                                       │   │
│  │                                                                     │   │
│  │  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────────┐     │   │
│  │  │    records      │  │ intervention_   │  │   query_stats   │     │   │
│  │  │  Vec<Record>    │  │     stats       │  │  HashMap<u64,   │     │   │
│  │  │                 │  │                 │  │   ContextStats> │     │   │
│  │  └─────────────────┘  └─────────────────┘  └─────────────────┘     │   │
│  │                                                                     │   │
│  │  ┌─────────────────┐                                               │   │
│  │  │   path_stats    │                                               │   │
│  │  │  HashMap<u64,   │                                               │   │
│  │  │   ContextStats> │                                               │   │
│  │  └─────────────────┘                                               │   │
│  │                                                                     │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
│  统计维度:                                                                   │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │  1. 按 InterventionPoint 聚合                                        │   │
│  │     - START / FORK / BACKTRACK / EVALUATE 各自的准确率               │   │
│  │                                                                     │   │
│  │  2. 按 Query 聚合                                                   │   │
│  │     - 相似查询的历史表现                                             │   │
│  │                                                                     │   │
│  │  3. 按 Path 聚合                                                    │   │
│  │     - 相似路径的历史表现                                             │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 2.3 PilotLearner - 学习器

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                          PilotLearner 工作原理                               │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  输入:                                                                       │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │  - intervention_point: 当前介入点类型                                 │   │
│  │  - query_hash: 查询的哈希值                                          │   │
│  │  - path_hash: 路径的哈希值                                           │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
│  查询历史统计:                                                               │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │  1. 获取 intervention_point 的整体准确率                              │   │
│  │  2. 获取 query_hash 的特定准确率（如有）                              │   │
│  │  3. 获取 path_hash 的特定准确率（如有）                               │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
│  输出 DecisionAdjustment:                                                   │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │  pub struct DecisionAdjustment {                                    │   │
│  │      /// 置信度调整（加到 Pilot 置信度上）                             │   │
│  │      pub confidence_delta: f64,                                     │   │
│  │      /// 是否跳过介入（信任算法）                                     │   │
│  │      pub skip_intervention: bool,                                   │   │
│  │      /// 算法权重 vs LLM 权重                                        │   │
│  │      pub algorithm_weight: f64,                                     │   │
│  │  }                                                                  │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## 3. 学习策略

### 3.1 准确率阈值

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                          准确率阈值策略                                       │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  配置参数:                                                                   │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │  min_samples: 10              // 最小样本数才开始调整                  │   │
│  │  high_accuracy_threshold: 0.8 // 高准确率阈值                         │   │
│  │  low_accuracy_threshold: 0.5  // 低准确率阈值                         │   │
│  │  max_confidence_delta: 0.2    // 最大置信度调整幅度                   │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
│  决策逻辑:                                                                   │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │                                                                     │   │
│  │  if accuracy >= high_accuracy_threshold (0.8):                      │   │
│  │      // 高准确率：信任 LLM，提升置信度                                 │   │
│  │      confidence_delta = +0.2                                        │   │
│  │      algorithm_weight = 0.3  // 更依赖 LLM                           │   │
│  │                                                                     │   │
│  │  elif accuracy <= low_accuracy_threshold (0.5):                     │   │
│  │      // 低准确率：信任算法，降低置信度                                 │   │
│  │      confidence_delta = -0.2                                        │   │
│  │      algorithm_weight = 0.7  // 更依赖算法                           │   │
│  │                                                                     │   │
│  │      if accuracy < 0.3:                                             │   │
│  │          // 非常低：跳过 LLM 调用，完全用算法                          │   │
│  │          skip_intervention = true                                   │   │
│  │                                                                     │   │
│  │  else:                                                              │   │
│  │      // 中等准确率：保持默认                                          │   │
│  │      confidence_delta = 0.0                                         │   │
│  │      algorithm_weight = 0.5                                         │   │
│  │                                                                     │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 3.2 多层统计融合

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                        多层统计融合策略                                       │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  三层统计:                                                                   │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │                                                                     │   │
│  │  Layer 1: InterventionPoint 级别（粗粒度）                           │   │
│  │  ┌─────────────────────────────────────────────────────────────┐   │   │
│  │  │  例如: FORK 点整体准确率 = 0.75                              │   │   │
│  │  │  影响: 基础调整                                              │   │   │
│  │  └─────────────────────────────────────────────────────────────┘   │   │
│  │                                                                     │   │
│  │  Layer 2: Query 级别（中粒度）                                       │   │
│  │  ┌─────────────────────────────────────────────────────────────┐   │   │
│  │  │  例如: 相似查询的准确率 = 0.85                               │   │   │
│  │  │  影响: 如果高于整体，额外 +0.05 置信度                        │   │   │
│  │  └─────────────────────────────────────────────────────────────┘   │   │
│  │                                                                     │   │
│  │  Layer 3: Path 级别（细粒度）                                        │   │
│  │  ┌─────────────────────────────────────────────────────────────┐   │   │
│  │  │  例如: 相似路径的准确率 = 0.92                               │   │   │
│  │  │  影响: 如果非常高，额外 +0.05 置信度                          │   │   │
│  │  └─────────────────────────────────────────────────────────────┘   │   │
│  │                                                                     │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
│  融合示例:                                                                   │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │                                                                     │   │
│  │  场景: FORK 点，相似查询，相似路径                                    │   │
│  │                                                                     │   │
│  │  1. FORK 整体准确率 0.75 → confidence_delta = +0.1                  │   │
│  │  2. 查询特定准确率 0.85 > 0.75 → confidence_delta += 0.05           │   │
│  │  3. 路径特定准确率 0.92 > 0.9 → confidence_delta += 0.05            │   │
│  │                                                                     │   │
│  │  最终: confidence_delta = +0.2 (达到上限)                           │   │
│  │                                                                     │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## 4. 与 LlmPilot 的集成

### 4.1 集成点

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                        LlmPilot 与 Learner 集成                              │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  LlmPilot 结构:                                                             │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │  pub struct LlmPilot {                                              │   │
│  │      client: LlmClient,                                             │   │
│  │      executor: Option<Arc<LlmExecutor>>,                            │   │
│  │      config: PilotConfig,                                           │   │
│  │      budget: BudgetController,                                      │   │
│  │      context_builder: ContextBuilder,                               │   │
│  │      prompt_builder: PromptBuilder,                                 │   │
│  │      response_parser: ResponseParser,                               │   │
│  │      learner: Option<Arc<PilotLearner>>,  // ← 反馈学习器            │   │
│  │  }                                                                  │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
│  关键方法:                                                                   │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │                                                                     │   │
│  │  // 添加学习器                                                      │   │
│  │  pub fn with_learner(self, learner: Arc<PilotLearner>) -> Self     │   │
│  │                                                                     │   │
│  │  // 从反馈存储创建学习器                                             │   │
│  │  pub fn with_feedback_store(self, store: Arc<FeedbackStore>) -> Self│   │
│  │                                                                     │   │
│  │  // 记录反馈                                                        │   │
│  │  pub fn record_feedback(&self, record: FeedbackRecord)             │   │
│  │                                                                     │   │
│  │  // 获取学习器（只读）                                               │   │
│  │  pub fn learner(&self) -> Option<&PilotLearner>                    │   │
│  │                                                                     │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 4.2 决策流程

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                          带学习的决策流程                                     │
└─────────────────────────────────────────────────────────────────────────────┘

                    ┌─────────────────┐
                    │  call_llm()     │
                    └────────┬────────┘
                             │
                             ▼
              ┌──────────────────────────────┐
              │  1. 构建上下文 (ContextBuilder) │
              │     - query_section           │
              │     - path_section            │
              │     - candidates_section      │
              └──────────────┬───────────────┘
                             │
                             ▼
              ┌──────────────────────────────┐
              │  2. 获取学习器调整             │
              │     if learner.is_some() {    │
              │       query_hash = ctx.hash() │
              │       path_hash = ctx.hash()  │
              │       adjustment = learner    │
              │         .get_adjustment(      │
              │           point,              │
              │           query_hash,         │
              │           path_hash           │
              │         )                     │
              │     }                         │
              └──────────────┬───────────────┘
                             │
                             ▼
              ┌──────────────────────────────┐
              │  3. 检查是否跳过介入           │
              │     if adjustment.skip {      │
              │       return default_decision │
              │     }                         │
              └──────────────┬───────────────┘
                             │
                             ▼
              ┌──────────────────────────────┐
              │  4. 调用 LLM 获取决策          │
              │     decision = llm.complete() │
              └──────────────┬───────────────┘
                             │
                             ▼
              ┌──────────────────────────────┐
              │  5. 应用学习器调整             │
              │     decision.confidence +=    │
              │       adjustment.confidence   │
              │       .delta                  │
              └──────────────┬───────────────┘
                             │
                             ▼
                    ┌─────────────────┐
                    │  返回调整后决策  │
                    └─────────────────┘
```

---

## 5. 使用示例

### 5.1 基本使用

```rust
use std::sync::Arc;
use vectorless::retrieval::pilot::{
    LlmPilot, PilotConfig,
    FeedbackStore, FeedbackRecord, PilotLearner,
};
use vectorless::llm::LlmClient;

// 1. 创建反馈存储
let store = Arc::new(FeedbackStore::in_memory());

// 2. 创建带学习器的 Pilot
let client = LlmClient::for_model("gpt-4o-mini");
let pilot = LlmPilot::new(client, PilotConfig::default())
    .with_feedback_store(store.clone());

// 3. 执行检索（Pilot 会自动应用学习调整）
let decision = pilot.decide(&state).await;

// 4. 记录用户反馈
let record = FeedbackRecord::new(
    decision_id,
    was_correct,  // 用户评价
    decision.confidence as f64,
    InterventionPoint::Fork,
    query_hash,
    path_hash,
);
pilot.record_feedback(record);

// 5. 后续检索会自动利用历史反馈改进决策
```

### 5.2 持久化反馈

```rust
use vectorless::retrieval::pilot::feedback::FeedbackStoreConfig;

// 创建带持久化的反馈存储
let config = FeedbackStoreConfig::with_persistence("./data/feedback.json");
let store = Arc::new(FeedbackStore::new(config));

// 启动时加载历史反馈
store.load()?;

// 定期保存
store.persist()?;
```

### 5.3 查看学习效果

```rust
// 获取整体准确率
let accuracy = learner.overall_accuracy();
println!("Overall accuracy: {:.2}%", accuracy * 100.0);

// 获取各介入点的统计
let stats = store.intervention_stats();
println!("Fork accuracy: {:.2}%", stats.fork.accuracy() * 100.0);
println!("Start accuracy: {:.2}%", stats.start.accuracy() * 100.0);

// 检查是否有足够数据
if learner.has_sufficient_data() {
    println!("Learner has sufficient data for adjustments");
}
```

---

## 6. 配置选项

```rust
/// 反馈存储配置
pub struct FeedbackStoreConfig {
    /// 最大记录数（内存限制）
    pub max_records: usize,
    /// 是否持久化
    pub persist: bool,
    /// 持久化路径
    pub storage_path: Option<String>,
}

/// 学习器配置
pub struct LearnerConfig {
    /// 最小样本数（少于此数不调整）
    pub min_samples: u64,
    /// 高准确率阈值
    pub high_accuracy_threshold: f64,
    /// 低准确率阈值
    pub low_accuracy_threshold: f64,
    /// 最大置信度调整幅度
    pub max_confidence_delta: f64,
}

impl Default for LearnerConfig {
    fn default() -> Self {
        Self {
            min_samples: 10,
            high_accuracy_threshold: 0.8,
            low_accuracy_threshold: 0.5,
            max_confidence_delta: 0.2,
        }
    }
}
```

---

## 7. 实现细节

### 7.1 哈希计算

```rust
impl PilotContext {
    /// 计算查询哈希（用于聚合相似查询）
    pub fn query_hash(&self) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut hasher = DefaultHasher::new();
        self.query_section.hash(&mut hasher);
        hasher.finish()
    }

    /// 计算路径哈希（用于聚合相似路径）
    pub fn path_hash(&self) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut hasher = DefaultHasher::new();
        self.path_section.hash(&mut hasher);
        hasher.finish()
    }
}
```

### 7.2 统计计算

```rust
impl ContextStats {
    /// 计算准确率
    pub fn accuracy(&self) -> f64 {
        if self.total == 0 {
            0.0
        } else {
            self.correct as f64 / self.total as f64
        }
    }

    /// 记录新反馈（增量更新）
    fn record(&mut self, was_correct: bool, confidence: f64) {
        self.total += 1;
        if was_correct {
            self.correct += 1;
            // 增量更新平均置信度
            self.avg_confidence_correct = 
                (self.avg_confidence_correct * (self.correct - 1) as f64 + confidence)
                / self.correct as f64;
        } else {
            let incorrect = self.total - self.correct;
            self.avg_confidence_incorrect = 
                (self.avg_confidence_incorrect * (incorrect - 1) as f64 + confidence)
                / incorrect as f64;
        }
    }
}
```

---

## 8. 未来扩展

### 8.1 可能的改进方向

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                          未来扩展方向                                         │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  1. 语义相似度聚合                                                          │
│     ┌─────────────────────────────────────────────────────────────────┐    │
│     │  当前: 使用精确哈希聚合                                          │    │
│     │  未来: 使用 embedding 计算语义相似度，聚合语义相近的查询           │    │
│     └─────────────────────────────────────────────────────────────────┘    │
│                                                                             │
│  2. 时间衰减                                                                │
│     ┌─────────────────────────────────────────────────────────────────┐    │
│     │  当前: 所有历史反馈等权重                                        │    │
│     │  未来: 近期反馈权重更高，旧反馈逐渐衰减                           │    │
│     └─────────────────────────────────────────────────────────────────┘    │
│                                                                             │
│  3. 在线学习                                                                │
│     ┌─────────────────────────────────────────────────────────────────┐    │
│     │  当前: 离线分析，在线应用                                        │    │
│     │  未来: 实时更新模型参数                                          │    │
│     └─────────────────────────────────────────────────────────────────┘    │
│                                                                             │
│  4. 个性化学习                                                              │
│     ┌─────────────────────────────────────────────────────────────────┐    │
│     │  当前: 全局学习                                                  │    │
│     │  未来: 按用户/场景分别学习                                       │    │
│     └─────────────────────────────────────────────────────────────────┘    │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## 9. 代码结构

```
src/retrieval/pilot/
├── mod.rs              # 模块入口
├── feedback.rs         # FeedbackStore, PilotLearner 实现
├── llm_pilot.rs        # LlmPilot（集成 learner）
├── builder.rs          # ContextBuilder（添加 hash 方法）
└── ...
```
