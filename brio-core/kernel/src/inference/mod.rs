//! Inference provider abstractions for the Brio kernel.
//!
//! This module provides a unified interface for interacting with
//! various LLM providers (Anthropic, OpenAI, etc.).

pub mod anthropic;
pub mod openai;
pub mod provider;
pub mod registry;
pub mod types;

pub use anthropic::{AnthropicConfig, AnthropicProvider};
pub use openai::{OpenAIConfig, OpenAIProvider};
pub use provider::LLMProvider;
pub use registry::ProviderRegistry;
pub use types::*;
