//! Anthropic API type mapping.
//!
//! This module provides types for mapping between internal and Anthropic API formats.

use crate::inference::types::{ChatResponse, Message, Role, Usage};
use serde::{Deserialize, Serialize};

/// Anthropic API message format
#[derive(Serialize)]
pub struct AnthropicMessage {
    /// The role of the message sender (user or assistant)
    pub role: String,
    /// The content of the message
    pub content: String,
}

/// Anthropic API chat request format
#[derive(Serialize)]
pub struct AnthropicChatRequest {
    /// The model identifier (e.g., "claude-3-opus-20240229")
    pub model: String,
    /// Maximum number of tokens to generate
    pub max_tokens: u32,
    /// The conversation messages
    pub messages: Vec<AnthropicMessage>,
    /// Optional system prompt
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system: Option<String>,
}

/// Anthropic API content block
#[derive(Deserialize)]
pub struct AnthropicContent {
    /// The text content of the response
    pub text: String,
}

/// Anthropic API usage information
#[derive(Deserialize)]
pub struct AnthropicUsage {
    /// Number of input tokens (prompt)
    pub input_tokens: u32,
    /// Number of output tokens (completion)
    pub output_tokens: u32,
}

/// Anthropic API chat response format
#[derive(Deserialize)]
pub struct AnthropicChatResponse {
    /// The generated content blocks
    pub content: Vec<AnthropicContent>,
    /// Token usage information if available
    pub usage: Option<AnthropicUsage>,
}

/// Converts internal Message type to Anthropic format, extracting system message
#[must_use]
pub fn prepare_messages(messages: &[Message]) -> (Option<String>, Vec<AnthropicMessage>) {
    let mut system_message = None;
    let mut anthropic_messages = Vec::with_capacity(messages.len());

    for msg in messages {
        match msg.role {
            Role::System => {
                // Anthropic uses a separate system field, not in messages array
                system_message = Some(msg.content.clone());
            }
            Role::User => {
                anthropic_messages.push(AnthropicMessage {
                    role: "user".to_string(),
                    content: msg.content.clone(),
                });
            }
            Role::Assistant => {
                anthropic_messages.push(AnthropicMessage {
                    role: "assistant".to_string(),
                    content: msg.content.clone(),
                });
            }
        }
    }

    (system_message, anthropic_messages)
}

/// Maps Anthropic API response to internal `ChatResponse`
#[must_use]
pub fn map_response(body: AnthropicChatResponse) -> ChatResponse {
    let content = body
        .content
        .first()
        .map(|c| c.text.clone())
        .unwrap_or_default();

    ChatResponse {
        content,
        usage: body.usage.map(|u| Usage {
            prompt_tokens: u.input_tokens,
            completion_tokens: u.output_tokens,
            total_tokens: u.input_tokens + u.output_tokens,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_prepare_messages_extracts_system() {
        let messages = vec![
            Message {
                role: Role::System,
                content: "You are helpful.".to_string(),
            },
            Message {
                role: Role::User,
                content: "Hello!".to_string(),
            },
        ];

        let (system, msgs) = prepare_messages(&messages);
        assert_eq!(system, Some("You are helpful.".to_string()));
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0].role, "user");
        assert_eq!(msgs[0].content, "Hello!");
    }

    #[test]
    fn test_prepare_messages_no_system() {
        let messages = vec![
            Message {
                role: Role::User,
                content: "Hello!".to_string(),
            },
            Message {
                role: Role::Assistant,
                content: "Hi there!".to_string(),
            },
        ];

        let (system, msgs) = prepare_messages(&messages);
        assert!(system.is_none());
        assert_eq!(msgs.len(), 2);
    }

    #[test]
    fn test_map_response() {
        let body = AnthropicChatResponse {
            content: vec![AnthropicContent {
                text: "Test response".to_string(),
            }],
            usage: Some(AnthropicUsage {
                input_tokens: 10,
                output_tokens: 20,
            }),
        };

        let response = map_response(body);
        assert_eq!(response.content, "Test response");
        assert!(response.usage.is_some());
        let usage = response.usage.unwrap();
        assert_eq!(usage.prompt_tokens, 10);
        assert_eq!(usage.completion_tokens, 20);
        assert_eq!(usage.total_tokens, 30);
    }

    #[test]
    fn test_map_response_no_usage() {
        let body = AnthropicChatResponse {
            content: vec![AnthropicContent {
                text: "No usage".to_string(),
            }],
            usage: None,
        };

        let response = map_response(body);
        assert_eq!(response.content, "No usage");
        assert!(response.usage.is_none());
    }

    #[test]
    fn test_map_response_empty_content() {
        let body = AnthropicChatResponse {
            content: vec![],
            usage: None,
        };

        let response = map_response(body);
        assert_eq!(response.content, "");
    }
}
