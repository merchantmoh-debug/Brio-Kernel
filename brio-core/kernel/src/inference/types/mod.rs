//! Type definitions for inference operations.
//!
//! This module contains shared types used across all LLM providers.

pub mod circuit_breaker;
pub mod error;
pub mod message;
pub mod request;
pub mod response;

// Re-export all types for convenience
pub use circuit_breaker::{
    CircuitBreaker, CircuitBreakerConfig, CircuitBreakerState, CircuitBreakerStats,
    DEFAULT_FAILURE_THRESHOLD, DEFAULT_HALF_OPEN_MAX_CALLS, DEFAULT_RESET_TIMEOUT_MS,
};
pub use error::InferenceError;
pub use message::{Message, Role};
pub use request::ChatRequest;
pub use response::{ChatResponse, Usage};
