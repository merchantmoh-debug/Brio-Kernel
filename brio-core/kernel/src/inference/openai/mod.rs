//! `OpenAI` API provider implementation.
//!
//! This module provides integration with `OpenAI`'s API for chat completions.

pub mod client;
pub mod mapping;
pub mod streaming;

pub use client::{OpenAIConfig, OpenAIProvider};
pub use mapping::{
    OpenAIChatRequest, OpenAIChatResponse, OpenAIChoice, OpenAIUsage, create_request, map_response,
};
pub use streaming::{
    DEFAULT_BASE_DELAY_MS, DEFAULT_MAX_RETRIES, MAX_DELAY_MS, RetryConfig, StreamingConfig,
    rand_jitter_factor,
};
