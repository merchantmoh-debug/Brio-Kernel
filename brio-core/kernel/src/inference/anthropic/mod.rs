//! Anthropic API provider implementation.
//!
//! This module provides integration with Anthropic's Claude API for chat completions.

pub mod client;
pub mod mapping;
pub mod retry;

pub use client::{AnthropicConfig, AnthropicProvider};
pub use mapping::{
    AnthropicChatRequest, AnthropicChatResponse, AnthropicContent, AnthropicMessage,
    AnthropicUsage, prepare_messages,
};
pub use retry::{
    DEFAULT_BASE_DELAY_MS, DEFAULT_MAX_RETRIES, MAX_DELAY_MS, RetryConfig, rand_jitter_factor,
};
