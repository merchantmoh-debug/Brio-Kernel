use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    System,
    User,
    Assistant,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: Role,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Usage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatResponse {
    pub content: String,
    pub usage: Option<Usage>,
}

#[derive(Debug, Clone)]
pub struct ChatRequest {
    pub model: String,
    pub messages: Vec<Message>,
}

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
    #[error("Provider Not Found: {0}")]
    ProviderNotFound(String),
}
