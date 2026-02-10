//! `OpenAI` API streaming support.
//!
//! This module provides retry mechanisms for streaming requests.
//! Future enhancements will include actual SSE (Server-Sent Events) streaming support.

use std::time::Duration;

/// Default maximum number of retries for transient errors
pub const DEFAULT_MAX_RETRIES: u32 = 3;
/// Default base delay for exponential backoff (in milliseconds)
pub const DEFAULT_BASE_DELAY_MS: u64 = 1000;
/// Maximum delay cap (in milliseconds)
pub const MAX_DELAY_MS: u64 = 30000;

/// Retry configuration for `OpenAI` provider
#[derive(Debug, Clone, Copy)]
pub struct RetryConfig {
    /// Maximum number of retry attempts for failed requests
    pub max_retries: u32,
    /// Base delay in milliseconds for exponential backoff
    pub base_delay_ms: u64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: DEFAULT_MAX_RETRIES,
            base_delay_ms: DEFAULT_BASE_DELAY_MS,
        }
    }
}

impl RetryConfig {
    /// Creates a new retry config with default values
    #[must_use]
    pub const fn new() -> Self {
        Self {
            max_retries: DEFAULT_MAX_RETRIES,
            base_delay_ms: DEFAULT_BASE_DELAY_MS,
        }
    }

    /// Sets the maximum number of retries
    #[must_use]
    pub const fn with_max_retries(mut self, max_retries: u32) -> Self {
        self.max_retries = max_retries;
        self
    }

    /// Sets the base delay for exponential backoff
    #[must_use]
    pub const fn with_base_delay_ms(mut self, delay_ms: u64) -> Self {
        self.base_delay_ms = delay_ms;
        self
    }

    /// Calculates the delay for a given retry attempt with jitter
    #[must_use]
    pub fn calculate_backoff_delay(&self, attempt: u32) -> Duration {
        // Exponential backoff: base_delay * 2^attempt
        let delay_ms = self.base_delay_ms.saturating_mul(1u64 << attempt);
        let capped_delay = delay_ms.min(MAX_DELAY_MS);

        // Add jitter (0-25% of the delay) using integer arithmetic
        // rand_jitter_factor returns value in [0, 1000]
        let jitter_factor = rand_jitter_factor();
        let jitter = capped_delay
            .saturating_mul(jitter_factor)
            .saturating_div(4000);
        Duration::from_millis(capped_delay.saturating_add(jitter))
    }
}

/// Simple pseudo-random jitter factor between 0 and 1000 (representing 0% to 100%)
#[must_use]
pub fn rand_jitter_factor() -> u64 {
    use std::time::SystemTime;
    let nanos = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0);
    u64::from(nanos % 1000)
}

/// Streaming configuration for `OpenAI` chat completions
#[derive(Debug, Clone)]
pub struct StreamingConfig {
    /// Whether to enable streaming
    pub enabled: bool,
    /// Timeout for each chunk in milliseconds
    pub chunk_timeout_ms: u64,
}

impl Default for StreamingConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            chunk_timeout_ms: 30000,
        }
    }
}

impl StreamingConfig {
    /// Creates a new streaming config with streaming disabled by default
    #[must_use]
    pub const fn new() -> Self {
        Self {
            enabled: false,
            chunk_timeout_ms: 30000,
        }
    }

    /// Enables streaming
    #[must_use]
    pub const fn with_enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// Sets the chunk timeout
    #[must_use]
    pub const fn with_chunk_timeout_ms(mut self, timeout_ms: u64) -> Self {
        self.chunk_timeout_ms = timeout_ms;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_retry_config_default() {
        let config = RetryConfig::default();
        assert_eq!(config.max_retries, DEFAULT_MAX_RETRIES);
        assert_eq!(config.base_delay_ms, DEFAULT_BASE_DELAY_MS);
    }

    #[test]
    fn test_retry_config_custom() {
        let config = RetryConfig::new()
            .with_max_retries(5)
            .with_base_delay_ms(500);
        assert_eq!(config.max_retries, 5);
        assert_eq!(config.base_delay_ms, 500);
    }

    #[test]
    fn test_backoff_delay_calculation() {
        let config = RetryConfig::new().with_base_delay_ms(1000);

        // First attempt: ~1000ms (+ jitter)
        let delay0 = config.calculate_backoff_delay(0);
        assert!(delay0.as_millis() >= 1000);
        assert!(delay0.as_millis() <= 1250); // 1000 + 25% jitter

        // Second attempt: ~2000ms (+ jitter)
        let delay1 = config.calculate_backoff_delay(1);
        assert!(delay1.as_millis() >= 2000);
        assert!(delay1.as_millis() <= 2500);
    }

    #[test]
    fn test_backoff_delay_caps_at_max() {
        let config = RetryConfig::new().with_base_delay_ms(10000);

        // With a high attempt number, should cap at MAX_DELAY_MS + 25% jitter
        let delay = config.calculate_backoff_delay(10);
        // Max delay is 30000 + 25% jitter = 37500
        let max_with_jitter = (u128::from(MAX_DELAY_MS) * 125) / 100;
        assert!(delay.as_millis() <= max_with_jitter);
    }

    #[test]
    fn test_jitter_factor_range() {
        // Test multiple times to ensure it's within range
        for _ in 0..10 {
            let factor = rand_jitter_factor();
            assert!(factor < 1000);
        }
    }

    #[test]
    fn test_streaming_config_default() {
        let config = StreamingConfig::default();
        assert!(!config.enabled);
        assert_eq!(config.chunk_timeout_ms, 30000);
    }

    #[test]
    fn test_streaming_config_custom() {
        let config = StreamingConfig::new()
            .with_enabled(true)
            .with_chunk_timeout_ms(60000);
        assert!(config.enabled);
        assert_eq!(config.chunk_timeout_ms, 60000);
    }
}
