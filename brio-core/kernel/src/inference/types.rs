use serde::{Deserialize, Serialize};

/// Role of the message sender.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    System,
    User,
    Assistant,
}

/// A chat message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: Role,
    pub content: String,
}

/// Token usage statistics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Usage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

/// Response from the LLM provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatResponse {
    pub content: String,
    pub usage: Option<Usage>,
}

/// Request parameters for chat completion.
#[derive(Debug, Clone)]
pub struct ChatRequest {
    pub model: String,
    pub messages: Vec<Message>,
}

/// Standardized error types for inference.
#[derive(Debug, thiserror::Error)]
pub enum InferenceError {
    #[error("Provider Error: {0}")]
    ProviderError(String),
    #[error("Rate Limit Exceeded")]
    RateLimit,
    #[error("Context Length Exceeded")]
    ContextLengthExceeded,
    #[error("Network Error: {0}")]
    NetworkError(String),
    #[error("Configuration Error: {0}")]
    ConfigError(String),
}
