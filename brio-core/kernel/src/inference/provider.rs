use crate::inference::types::{ChatRequest, ChatResponse, InferenceError};
use async_trait::async_trait;

#[async_trait]
pub trait LLMProvider: Send + Sync {
    /// Executes a chat completion request.
    async fn chat(&self, request: ChatRequest) -> Result<ChatResponse, InferenceError>;
}
