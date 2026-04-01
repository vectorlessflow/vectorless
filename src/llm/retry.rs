// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Retry logic for LLM calls.

use std::future::Future;
use tracing::{debug, warn};

use super::config::RetryConfig;
use super::error::{LlmError, LlmResult};

/// Execute an async operation with retry logic.
///
/// This function implements exponential backoff retry for operations
/// that may fail with transient errors (rate limits, timeouts, etc.).
///
/// # Example
///
/// ```rust,no_run
/// use vectorless::llm::{RetryConfig, with_retry, LlmError, LlmResult};
///
/// # #[tokio::main]
/// # async fn main() -> LlmResult<()> {
/// let config = RetryConfig::default();
///
/// let result = with_retry(&config, || async {
///     // Some operation that might fail
///     Ok::<_, LlmError>("success".to_string())
/// }).await?;
///
/// # Ok(())
/// # }
/// ```
pub async fn with_retry<F, Fut, T>(
    config: &RetryConfig,
    operation: F,
) -> LlmResult<T>
where
    F: Fn() -> Fut,
    Fut: Future<Output = LlmResult<T>>,
{
    let mut attempts = 0;

    loop {
        attempts += 1;

        match operation().await {
            Ok(result) => {
                if attempts > 1 {
                    debug!("Retry succeeded on attempt {}", attempts);
                }
                return Ok(result);
            }
            Err(e) => {
                // Check if we should retry
                if !should_retry(&e, config) {
                    return Err(e);
                }

                // Check if we've exhausted retries
                if attempts >= config.max_attempts {
                    warn!(
                        attempts = attempts,
                        max_attempts = config.max_attempts,
                        "Retry exhausted"
                    );
                    return Err(LlmError::RetryExhausted {
                        attempts,
                        last_error: e.to_string(),
                    });
                }

                // Calculate delay for this attempt (0-indexed for delay calculation)
                let delay = config.delay_for_attempt(attempts - 1);
                warn!(
                    attempt = attempts,
                    max_attempts = config.max_attempts,
                    delay_ms = delay.as_millis() as u64,
                    error = %e,
                    "LLM call failed, retrying..."
                );

                tokio::time::sleep(delay).await;
            }
        }
    }
}

/// Determine if an error should trigger a retry.
fn should_retry(error: &LlmError, config: &RetryConfig) -> bool {
    match error {
        LlmError::RateLimit(_) => config.retry_on_rate_limit,
        LlmError::Timeout(_) => true,
        LlmError::Api(msg) => {
            let msg_lower = msg.to_lowercase();
            // Check for retryable API errors
            msg_lower.contains("rate limit") ||
            msg_lower.contains("429") ||
            msg_lower.contains("503") ||
            msg_lower.contains("502") ||
            msg_lower.contains("timeout") ||
            msg_lower.contains("overloaded")
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    #[tokio::test]
    async fn test_retry_success_on_second_attempt() {
        let config = RetryConfig::new().with_max_attempts(3);
        let attempts = AtomicU32::new(0);

        let result = with_retry(&config, || async {
            let current = attempts.fetch_add(1, Ordering::SeqCst) + 1;
            if current < 2 {
                Err(LlmError::Timeout("timeout".to_string()))
            } else {
                Ok("success")
            }
        }).await;

        assert_eq!(result.unwrap(), "success");
        assert_eq!(attempts.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn test_retry_max_attempts_reached() {
        let config = RetryConfig::new().with_max_attempts(2);
        let attempts = AtomicU32::new(0);

        let result: LlmResult<String> = with_retry(&config, || async {
            attempts.fetch_add(1, Ordering::SeqCst);
            Err(LlmError::Timeout("timeout".to_string()))
        }).await;

        assert!(matches!(result, Err(LlmError::RetryExhausted { .. })));
        assert_eq!(attempts.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn test_non_retryable_error_fails_immediately() {
        let config = RetryConfig::new().with_max_attempts(3);
        let attempts = AtomicU32::new(0);

        let result: LlmResult<String> = with_retry(&config, || async {
            attempts.fetch_add(1, Ordering::SeqCst);
            Err(LlmError::Config("bad config".to_string()))
        }).await;

        assert!(matches!(result, Err(LlmError::Config(_))));
        assert_eq!(attempts.load(Ordering::SeqCst), 1); // Should only try once
    }
}
