//! Response types for inference operations.
//!
//! This module contains response and usage definitions for chat completions.

use serde::{Deserialize, Serialize};

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

impl ChatResponse {
    /// Creates a new chat response
    #[must_use]
    pub fn new(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            usage: None,
        }
    }

    /// Creates a new chat response with usage information
    #[must_use]
    pub fn with_usage(
        content: impl Into<String>,
        prompt_tokens: u32,
        completion_tokens: u32,
    ) -> Self {
        let prompt = prompt_tokens;
        let completion = completion_tokens;
        Self {
            content: content.into(),
            usage: Some(Usage {
                prompt_tokens: prompt,
                completion_tokens: completion,
                total_tokens: prompt + completion,
            }),
        }
    }

    /// Returns the content length in characters
    #[must_use]
    pub fn content_length(&self) -> usize {
        self.content.len()
    }

    /// Returns true if the response includes usage information
    #[must_use]
    pub fn has_usage(&self) -> bool {
        self.usage.is_some()
    }

    /// Returns the total token count if available
    #[must_use]
    pub fn total_tokens(&self) -> Option<u32> {
        self.usage.as_ref().map(|u| u.total_tokens)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chat_response_new() {
        let response = ChatResponse::new("Hello, world!");
        assert_eq!(response.content, "Hello, world!");
        assert!(response.usage.is_none());
    }

    #[test]
    fn test_chat_response_with_usage() {
        let response = ChatResponse::with_usage("Test response", 10, 20);
        assert_eq!(response.content, "Test response");
        assert!(response.usage.is_some());

        let usage = response.usage.unwrap();
        assert_eq!(usage.prompt_tokens, 10);
        assert_eq!(usage.completion_tokens, 20);
        assert_eq!(usage.total_tokens, 30);
    }

    #[test]
    fn test_content_length() {
        let response = ChatResponse::new("Hello");
        assert_eq!(response.content_length(), 5);
    }

    #[test]
    fn test_has_usage() {
        let response_without = ChatResponse::new("No usage");
        assert!(!response_without.has_usage());

        let response_with = ChatResponse::with_usage("With usage", 5, 10);
        assert!(response_with.has_usage());
    }

    #[test]
    fn test_total_tokens() {
        let response_without = ChatResponse::new("No usage");
        assert!(response_without.total_tokens().is_none());

        let response_with = ChatResponse::with_usage("With usage", 5, 10);
        assert_eq!(response_with.total_tokens(), Some(15));
    }

    #[test]
    fn test_usage_serialization() {
        let usage = Usage {
            prompt_tokens: 10,
            completion_tokens: 20,
            total_tokens: 30,
        };
        let json = serde_json::to_string(&usage).unwrap();
        assert!(json.contains("\"prompt_tokens\":10"));
        assert!(json.contains("\"completion_tokens\":20"));
        assert!(json.contains("\"total_tokens\":30"));
    }

    #[test]
    fn test_chat_response_serialization() {
        let response = ChatResponse::with_usage("Test", 5, 10);
        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("\"content\":\"Test\""));
        assert!(json.contains("\"prompt_tokens\":5"));
    }
}
