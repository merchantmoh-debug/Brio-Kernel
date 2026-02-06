//! LLM provider trait definition.
//!
//! This module defines the common interface for all LLM providers.

use crate::inference::types::{ChatRequest, ChatResponse, InferenceError};
use async_trait::async_trait;

/// A trait for LLM providers that can execute chat completion requests.
#[async_trait]
pub trait LLMProvider: Send + Sync {
    /// Executes a chat completion request.
    async fn chat(&self, request: ChatRequest) -> Result<ChatResponse, InferenceError>;
}
