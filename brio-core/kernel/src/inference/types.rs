//! Type definitions for inference operations.
//!
//! This module contains shared types used across all LLM providers.

use serde::{Deserialize, Serialize};

/// The role of a message in a conversation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    /// System-level instructions
    System,
    /// User input
    User,
    /// Assistant response
    Assistant,
}

/// A message in a conversation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    /// The role of the message sender
    pub role: Role,
    /// The content of the message
    pub content: String,
}

/// Token usage information for a completion request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Usage {
    /// Number of tokens in the prompt
    pub prompt_tokens: u32,
    /// Number of tokens in the completion
    pub completion_tokens: u32,
    /// Total number of tokens used
    pub total_tokens: u32,
}

/// Response from a chat completion request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatResponse {
    /// The generated content
    pub content: String,
    /// Token usage information, if available
    pub usage: Option<Usage>,
}

/// Request for a chat completion.
#[derive(Debug, Clone)]
pub struct ChatRequest {
    /// The model to use for completion
    pub model: String,
    /// The conversation history
    pub messages: Vec<Message>,
}

/// Errors that can occur during inference operations.
#[derive(Debug, thiserror::Error)]
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
}
