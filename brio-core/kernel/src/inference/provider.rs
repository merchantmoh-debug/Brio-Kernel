use crate::inference::types::{ChatRequest, ChatResponse, InferenceError};
use async_trait::async_trait;

/// Pluggable interface for LLM Providers.
///
/// Follows Dependency Inversion (DIP) and Open/Closed Principle (OCP).
/// The Host depends on this trait, not specific implementations.
#[async_trait]
pub trait LLMProvider: Send + Sync {
    /// Executes a chat completion request.
    /// Argument is a specific DTO to avoid long parameter lists (Clean Code).
    async fn chat(&self, request: ChatRequest) -> Result<ChatResponse, InferenceError>;
}
