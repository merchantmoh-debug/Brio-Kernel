//! Error types for inference operations.
//!
//! This module contains error definitions for LLM inference.

/// Errors that can occur during inference operations.
#[derive(Debug, Clone, thiserror::Error)]
pub enum InferenceError {
    /// Error from the LLM provider
    #[error("Provider Error: {0}")]
    ProviderError(String),
    /// Rate limit exceeded
    #[error("Rate Limit Exceeded")]
    RateLimit,
    /// Context length exceeded the model's limit
    #[error("Context Length Exceeded")]
    ContextLengthExceeded,
    /// Network error during request
    #[error("Network Error: {0}")]
    NetworkError(String),
    /// Configuration error
    #[error("Configuration Error: {0}")]
    ConfigError(String),
    /// Provider not found in registry
    #[error("Provider Not Found: {0}")]
    ProviderNotFound(String),
    /// Circuit breaker is open
    #[error("Circuit Breaker Open: {0}")]
    CircuitBreakerOpen(String),
    /// All providers in chain failed
    #[error("All Providers Failed")]
    AllProvidersFailed,
}

impl InferenceError {
    /// Returns `true` if this error is transient and retry may succeed.
    #[must_use]
    pub fn is_retryable(&self) -> bool {
        match self {
            Self::RateLimit => true,
            Self::NetworkError(_) => true,
            Self::ProviderError(msg) => {
                // Retry on transient HTTP errors
                msg.contains("HTTP 50") || msg.contains("HTTP 52")
            }
            Self::CircuitBreakerOpen(_) => false,
            Self::AllProvidersFailed => false,
            Self::ContextLengthExceeded => false,
            Self::ConfigError(_) => false,
            Self::ProviderNotFound(_) => false,
        }
    }

    /// Returns `true` if this error is permanent and should not be retried.
    #[must_use]
    pub fn is_permanent(&self) -> bool {
        !self.is_retryable()
    }

    /// Returns `true` if this error is a circuit breaker error.
    #[must_use]
    pub fn is_circuit_breaker(&self) -> bool {
        matches!(self, Self::CircuitBreakerOpen(_))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_is_retryable() {
        assert!(InferenceError::RateLimit.is_retryable());
        assert!(InferenceError::NetworkError("timeout".to_string()).is_retryable());
        assert!(InferenceError::ProviderError("HTTP 503".to_string()).is_retryable());
        assert!(InferenceError::ProviderError("HTTP 500".to_string()).is_retryable());

        assert!(!InferenceError::CircuitBreakerOpen("test".to_string()).is_retryable());
        assert!(!InferenceError::AllProvidersFailed.is_retryable());
        assert!(!InferenceError::ContextLengthExceeded.is_retryable());
        assert!(!InferenceError::ConfigError("invalid".to_string()).is_retryable());
        assert!(!InferenceError::ProviderNotFound("missing".to_string()).is_retryable());
    }

    #[test]
    fn test_error_is_permanent() {
        assert!(InferenceError::ContextLengthExceeded.is_permanent());
        assert!(!InferenceError::RateLimit.is_permanent());
        assert!(!InferenceError::NetworkError("test".to_string()).is_permanent());
    }

    #[test]
    fn test_error_is_circuit_breaker() {
        assert!(InferenceError::CircuitBreakerOpen("test".to_string()).is_circuit_breaker());
        assert!(!InferenceError::RateLimit.is_circuit_breaker());
        assert!(!InferenceError::AllProvidersFailed.is_circuit_breaker());
    }

    #[test]
    fn test_error_display() {
        let err = InferenceError::ProviderError("API down".to_string());
        assert_eq!(err.to_string(), "Provider Error: API down");

        let err = InferenceError::RateLimit;
        assert_eq!(err.to_string(), "Rate Limit Exceeded");

        let err = InferenceError::CircuitBreakerOpen("test".to_string());
        assert_eq!(err.to_string(), "Circuit Breaker Open: test");
    }
}
