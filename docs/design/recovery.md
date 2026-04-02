# P0 Error Recovery 优雅降级策略

## 概述

当 LLM API 调用遇到各种错误时，系统能够智能地处理并降级到可用的备选方案，而不是直接崩溃或失败。

## 核心场景

| 错误类型 | 降级策略 |
|---------|---------|
| **Rate Limit (429)** | 等待后重试，或切换到备用模型 |
| **Model Not Available** | 自动切换到备用模型 (如 gpt-4o → gpt-4o-mini) |
| **API Key 失效** | 尝试其他配置的 API Key |
| **Endpoint 故障** | 切换到备用 Endpoint |
| **Timeout** | 减少输入长度后重试 |
| **所有尝试失败** | 返回缓存结果或默认值（如果允许） |

## 降级流程

```
用户请求 → 调用 gpt-4o
              ↓
         429 Rate Limit
              ↓
         等待 + 重试 (最多3次)
              ↓
         仍然失败?
              ↓
         降级到 gpt-4o-mini
              ↓
         成功 ✓  (标记结果为 degraded=true)
```

## 配置设计

```toml
[llm.fallback]
enabled = true

# 备用模型链（按优先级排列）
models = ["gpt-4o", "gpt-4o-mini", "glm-4-flash"]

# 备用端点
endpoints = [
    "https://api.openai.com/v1",
    "https://api.z.ai/api/paas/v4"
]

# 降级行为配置
on_rate_limit = "retry_then_fallback"  # retry, fallback, fail
on_timeout = "truncate_and_retry"
on_all_failed = "return_error"  # return_error, return_cache, return_default
```

## 实现计划

### Phase 1: 配置结构

1. 在 `src/config/types.rs` 添加 `FallbackConfig`
2. 在 `src/llm/error.rs` 完善错误分类

### Phase 2: 降级核心逻辑

1. 创建 `src/llm/fallback.rs` 模块
2. 实现 `FallbackChain` 结构体
3. 实现模型切换逻辑
4. 实现端点切换逻辑

### Phase 3: 集成

1. 在 `LlmClient` 中集成 `FallbackChain`
2. 更新配置文件加载逻辑
3. 添加 `degraded` 标记到响应

### Phase 4: 测试

1. 单元测试：各降级场景
2. 集成测试：端到端降级流程

## 代码结构

```
src/llm/
├── mod.rs           # 模块导出
├── client.rs        # LLM 客户端（集成 fallback）
├── config.rs        # LLM 配置
├── error.rs         # 错误类型（含分类）
├── retry.rs         # 重试逻辑
├── fallback.rs      # 降级逻辑（新增）
└── pool.rs          # 客户端池
```

## API 设计

### FallbackConfig

```rust
pub struct FallbackConfig {
    /// 是否启用降级
    pub enabled: bool,

    /// 备用模型列表（按优先级）
    pub models: Vec<String>,

    /// 备用端点列表
    pub endpoints: Vec<String>,

    /// Rate limit 时的行为
    pub on_rate_limit: FallbackBehavior,

    /// Timeout 时的行为
    pub on_timeout: FallbackBehavior,

    /// 全部失败时的行为
    pub on_all_failed: OnAllFailedBehavior,
}

pub enum FallbackBehavior {
    Retry,
    Fallback,
    RetryThenFallback,
    Fail,
}

pub enum OnAllFailedBehavior {
    ReturnError,
    ReturnCache,
}
```

### FallbackResult

```rust
pub struct FallbackResult<T> {
    /// 实际返回的结果
    pub result: T,

    /// 是否经过降级
    pub degraded: bool,

    /// 最终使用的模型
    pub model: String,

    /// 最终使用的端点
    pub endpoint: String,

    /// 降级历史（用于调试）
    pub fallback_history: Vec<FallbackStep>,
}

pub struct FallbackStep {
    pub from_model: String,
    pub to_model: Option<String>,
    pub from_endpoint: String,
    pub to_endpoint: Option<String>,
    pub reason: String,
}
```

## 错误分类

```rust
impl LlmError {
    /// 判断错误是否可重试
    pub fn is_retryable(&self) -> bool {
        matches!(self,
            LlmError::RateLimit(_) |
            LlmError::Timeout(_) |
            LlmError::Api(msg) if is_transient_error(msg)
        )
    }

    /// 判断错误是否应触发降级
    pub fn should_fallback(&self) -> bool {
        matches!(self,
            LlmError::RateLimit(_) |
            LlmError::Api(msg) if is_model_unavailable(msg)
        )
    }
}
```
