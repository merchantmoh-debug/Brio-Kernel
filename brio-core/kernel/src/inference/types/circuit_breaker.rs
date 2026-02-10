//! Circuit breaker implementation for resilience.
//!
//! This module provides circuit breaker pattern implementation to prevent
//! cascading failures in the inference system.

use std::time::{Duration, Instant};

/// Default failure threshold before circuit breaker opens
pub const DEFAULT_FAILURE_THRESHOLD: u32 = 5;
/// Default reset timeout in milliseconds
pub const DEFAULT_RESET_TIMEOUT_MS: u64 = 30_000;
/// Default half-open max test calls
pub const DEFAULT_HALF_OPEN_MAX_CALLS: u32 = 3;

/// Configuration for circuit breaker behavior.
#[derive(Debug, Clone, Copy)]
pub struct CircuitBreakerConfig {
    /// Number of failures before opening the circuit
    pub failure_threshold: u32,
    /// Time in milliseconds to wait before attempting reset
    pub reset_timeout_ms: u64,
    /// Maximum number of test calls in half-open state
    pub half_open_max_calls: u32,
}

impl CircuitBreakerConfig {
    /// Creates a new config with default values.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            failure_threshold: DEFAULT_FAILURE_THRESHOLD,
            reset_timeout_ms: DEFAULT_RESET_TIMEOUT_MS,
            half_open_max_calls: DEFAULT_HALF_OPEN_MAX_CALLS,
        }
    }

    /// Sets the failure threshold.
    #[must_use]
    pub const fn with_failure_threshold(mut self, threshold: u32) -> Self {
        self.failure_threshold = threshold;
        self
    }

    /// Sets the reset timeout in milliseconds.
    #[must_use]
    pub const fn with_reset_timeout_ms(mut self, timeout_ms: u64) -> Self {
        self.reset_timeout_ms = timeout_ms;
        self
    }

    /// Sets the max test calls for half-open state.
    #[must_use]
    pub const fn with_half_open_max_calls(mut self, max_calls: u32) -> Self {
        self.half_open_max_calls = max_calls;
        self
    }
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self::new()
    }
}

/// The state of a circuit breaker.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CircuitBreakerState {
    /// Circuit is closed, requests flow through normally
    #[default]
    Closed,
    /// Circuit is open, requests fail fast
    Open,
    /// Circuit is testing recovery with limited traffic
    HalfOpen,
}

/// Circuit breaker statistics for monitoring.
#[derive(Debug, Clone, Default)]
pub struct CircuitBreakerStats {
    /// Current state of the circuit
    pub state: CircuitBreakerState,
    /// Number of consecutive failures
    pub consecutive_failures: u32,
    /// Total number of successes
    pub total_successes: u64,
    /// Total number of failures
    pub total_failures: u64,
    /// Last state change time
    pub last_state_change: Option<Instant>,
    /// Number of calls allowed in half-open state
    pub half_open_calls_remaining: u32,
}

/// A circuit breaker for preventing cascading failures.
pub struct CircuitBreaker {
    config: CircuitBreakerConfig,
    state: CircuitBreakerState,
    consecutive_failures: u32,
    total_successes: u64,
    total_failures: u64,
    last_failure_time: Option<Instant>,
    half_open_calls_remaining: u32,
}

impl CircuitBreaker {
    /// Creates a new circuit breaker with the given configuration.
    #[must_use]
    pub const fn new(config: CircuitBreakerConfig) -> Self {
        Self {
            config,
            state: CircuitBreakerState::Closed,
            consecutive_failures: 0,
            total_successes: 0,
            total_failures: 0,
            last_failure_time: None,
            half_open_calls_remaining: 0,
        }
    }

    /// Attempts to acquire permission to execute a call.
    /// Returns `true` if the call should proceed, `false` if circuit is open.
    pub fn try_acquire(&mut self) -> bool {
        match self.state {
            CircuitBreakerState::Closed => true,
            CircuitBreakerState::Open => {
                // Check if we should transition to half-open
                if let Some(last_failure) = self.last_failure_time {
                    let elapsed = last_failure.elapsed();
                    let timeout = Duration::from_millis(self.config.reset_timeout_ms);
                    if elapsed >= timeout {
                        self.transition_to_half_open();
                        return true;
                    }
                }
                false
            }
            CircuitBreakerState::HalfOpen => {
                if self.half_open_calls_remaining > 0 {
                    self.half_open_calls_remaining -= 1;
                    true
                } else {
                    false
                }
            }
        }
    }

    /// Records a successful call.
    pub fn record_success(&mut self) {
        self.consecutive_failures = 0;
        self.total_successes += 1;

        if self.state == CircuitBreakerState::HalfOpen {
            // Transition back to closed on success in half-open state
            self.transition_to_closed();
        }
    }

    /// Records a failed call.
    pub fn record_failure(&mut self) {
        self.consecutive_failures += 1;
        self.total_failures += 1;
        self.last_failure_time = Some(Instant::now());

        match self.state {
            CircuitBreakerState::Closed => {
                if self.consecutive_failures >= self.config.failure_threshold {
                    self.transition_to_open();
                }
            }
            CircuitBreakerState::HalfOpen => {
                // Any failure in half-open state opens the circuit again
                self.transition_to_open();
            }
            CircuitBreakerState::Open => {
                // Already open, update last failure time
            }
        }
    }

    /// Returns the current state.
    #[must_use]
    pub const fn state(&self) -> CircuitBreakerState {
        self.state
    }

    /// Returns current statistics.
    #[must_use]
    pub fn stats(&self) -> CircuitBreakerStats {
        CircuitBreakerStats {
            state: self.state,
            consecutive_failures: self.consecutive_failures,
            total_successes: self.total_successes,
            total_failures: self.total_failures,
            last_state_change: self.last_failure_time,
            half_open_calls_remaining: self.half_open_calls_remaining,
        }
    }

    /// Force the circuit breaker to closed state.
    pub fn force_close(&mut self) {
        self.transition_to_closed();
    }

    /// Force the circuit breaker to open state.
    pub fn force_open(&mut self) {
        self.transition_to_open();
    }

    fn transition_to_open(&mut self) {
        self.state = CircuitBreakerState::Open;
        self.half_open_calls_remaining = 0;
    }

    fn transition_to_half_open(&mut self) {
        self.state = CircuitBreakerState::HalfOpen;
        self.half_open_calls_remaining = self.config.half_open_max_calls;
        self.consecutive_failures = 0;
    }

    fn transition_to_closed(&mut self) {
        self.state = CircuitBreakerState::Closed;
        self.consecutive_failures = 0;
        self.half_open_calls_remaining = 0;
    }
}

impl Default for CircuitBreaker {
    fn default() -> Self {
        Self::new(CircuitBreakerConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_circuit_breaker_starts_closed() {
        let mut cb = CircuitBreaker::default();
        assert_eq!(cb.state(), CircuitBreakerState::Closed);
        assert!(cb.try_acquire());
    }

    #[test]
    fn test_circuit_breaker_opens_after_failures() {
        let config = CircuitBreakerConfig::new()
            .with_failure_threshold(3)
            .with_reset_timeout_ms(1000);
        let mut cb = CircuitBreaker::new(config);

        assert!(cb.try_acquire());
        cb.record_failure();
        assert!(cb.try_acquire());
        cb.record_failure();
        assert!(cb.try_acquire());
        cb.record_failure();

        // Should now be open
        assert_eq!(cb.state(), CircuitBreakerState::Open);
        assert!(!cb.try_acquire());
    }

    #[test]
    fn test_circuit_breaker_success_resets_failures() {
        let config = CircuitBreakerConfig::new().with_failure_threshold(3);
        let mut cb = CircuitBreaker::new(config);

        cb.record_failure();
        cb.record_failure();
        cb.record_success();
        cb.record_failure();
        cb.record_failure();

        // Should still be closed since failures were reset
        assert_eq!(cb.state(), CircuitBreakerState::Closed);
        assert!(cb.try_acquire());
    }

    #[test]
    fn test_circuit_breaker_forced_states() {
        let mut cb = CircuitBreaker::default();

        cb.force_open();
        assert_eq!(cb.state(), CircuitBreakerState::Open);
        assert!(!cb.try_acquire());

        cb.force_close();
        assert_eq!(cb.state(), CircuitBreakerState::Closed);
        assert!(cb.try_acquire());
    }

    #[test]
    fn test_circuit_breaker_stats() {
        let mut cb = CircuitBreaker::default();

        cb.record_failure();
        cb.record_failure();
        cb.record_success();

        let stats = cb.stats();
        assert_eq!(stats.state, CircuitBreakerState::Closed);
        assert_eq!(stats.consecutive_failures, 0);
        assert_eq!(stats.total_successes, 1);
        assert_eq!(stats.total_failures, 2);
    }

    #[test]
    fn test_circuit_breaker_config_default() {
        let config = CircuitBreakerConfig::default();
        assert_eq!(config.failure_threshold, DEFAULT_FAILURE_THRESHOLD);
        assert_eq!(config.reset_timeout_ms, DEFAULT_RESET_TIMEOUT_MS);
        assert_eq!(config.half_open_max_calls, DEFAULT_HALF_OPEN_MAX_CALLS);
    }

    #[test]
    fn test_circuit_breaker_config_custom() {
        let config = CircuitBreakerConfig::new()
            .with_failure_threshold(10)
            .with_reset_timeout_ms(60000)
            .with_half_open_max_calls(5);

        assert_eq!(config.failure_threshold, 10);
        assert_eq!(config.reset_timeout_ms, 60000);
        assert_eq!(config.half_open_max_calls, 5);
    }
}
